// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeSet,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::mpsc::channel,
};

use anyhow::{anyhow, Result};
use clap::Parser;
use notify::{RecursiveMode, Watcher};
use sui_sdk::rpc_types::{
    SuiExecutionStatus,
    SuiTransactionBlockEffects,
    SuiTransactionBlockResponse,
};
use sui_types::{
    base_types::{ObjectID, SuiAddress},
    Identifier,
};

use crate::{
    display,
    preprocessor::Preprocessor,
    publish::WhenWalrusUpload::Modified,
    site::{
        builder::SitePtb,
        config::WSResources,
        manager::{SiteIdentifier, SiteIdentifier::ExistingSite, SiteManager},
        resource::ResourceManager,
        RemoteSiteFactory,
        SITE_MODULE,
    },
    summary::{SiteDataDiffSummary, Summarizable},
    util::{
        get_site_id_from_response,
        id_to_base36,
        load_wallet_context,
        path_or_defaults_if_exist,
        sign_and_send_ptb,
    },
    walrus::Walrus,
    Config,
};

const DEFAULT_WS_RESOURCES_FILE: &str = "ws-resources.json";

#[derive(Parser, Debug, Clone)]
pub struct PublishOptions {
    /// The directory containing the site sources.
    pub directory: PathBuf,
    /// The path to the Walrus sites resources file.
    ///
    /// This JSON configuration file defined HTTP resource headers and other utilities for your
    /// files. By default, the file is expected to be named `ws-resources.json` and located in the
    /// root of the site directory.
    ///
    /// The configuration file _will not_ be uploaded to Walrus.
    #[clap(long)]
    ws_resources: Option<PathBuf>,
    /// The number of epochs for which to save the resources on Walrus.
    #[clap(long, default_value_t = 1)]
    pub epochs: u64,
    /// Preprocess the directory before publishing.
    /// See the `list-directory` command. Warning: Rewrites all `index.html` files.
    #[clap(long, action)]
    pub list_directory: bool,
    /// The maximum number of concurrent calls to the Walrus CLI for the computation of blob IDs.
    #[clap(long)]
    max_concurrent: Option<NonZeroUsize>,

    /// By default, sites are deletable with site-builder delete command. By passing --permanent, the site is deleted only after `epochs` expiration.
    #[clap(long)]
    permanent: Option<bool>,
}

/// The continuous editing options.
#[derive(Debug, Clone)]
pub(crate) enum ContinuousEditing {
    /// Edit the site once and exit.
    Once,
    /// Watch the directory for changes and publish the site on change.
    Watch,
}

impl ContinuousEditing {
    /// Convert the flag to the enum.
    pub fn from_watch_flag(flag: bool) -> Self {
        if flag {
            ContinuousEditing::Watch
        } else {
            ContinuousEditing::Once
        }
    }
}

/// Force the update of walrus blobs.
#[derive(Debug, Clone)]
pub(crate) enum WhenWalrusUpload {
    /// Force the update of walrus blobs.
    Always,
    /// Only update modified
    Modified,
}

impl WhenWalrusUpload {
    pub fn is_always(&self) -> bool {
        matches!(self, WhenWalrusUpload::Always)
    }
}

/// When to upload the resources to Walrus.
impl WhenWalrusUpload {
    pub fn from_force_flag(force: bool) -> Self {
        if force {
            WhenWalrusUpload::Always
        } else {
            WhenWalrusUpload::Modified
        }
    }
}

pub(crate) struct EditOptions {
    pub publish_options: PublishOptions,
    pub site_id: SiteIdentifier,
    pub continuous_editing: ContinuousEditing,
    pub when_upload: WhenWalrusUpload,
}

pub(crate) struct SiteEditor<E = ()> {
    config: Config,
    edit_options: E,
}

impl SiteEditor {
    pub fn new(config: Config) -> Self {
        SiteEditor {
            config,
            edit_options: (),
        }
    }

    pub fn with_edit_options(
        self,
        publish_options: PublishOptions,
        site_id: SiteIdentifier,
        continuous_editing: ContinuousEditing,
        when_upload: WhenWalrusUpload,
    ) -> SiteEditor<EditOptions> {
        SiteEditor {
            config: self.config,
            edit_options: EditOptions {
                publish_options,
                site_id,
                continuous_editing,
                when_upload,
            },
        }
    }

    pub async fn destroy(&self, site_id: ObjectID) -> Result<()> {
        // Delete blobs on Walrus
        let wallet_walrus = load_wallet_context(&self.config.general.wallet)?;

        let all_dynamic_fields =
            RemoteSiteFactory::new(&wallet_walrus.get_client().await?, self.config.package)
                .await?
                .get_existing_resources(site_id)
                .await?;

        let walrus = Walrus::new(
            self.config.walrus_binary(),
            self.config.gas_budget(),
            self.config.general.rpc_url.clone(),
            self.config.general.walrus_config.clone(),
            self.config.general.wallet.clone(),
        );
        let mut site_manager = SiteManager::new(
            self.config.clone(),
            walrus,
            wallet_walrus,
            ExistingSite(site_id),
            0,
            Modified,
            false,
        )
        .await?;

        tracing::debug!(
            "Retrieved blobs and deleting them: {:?}",
            &all_dynamic_fields,
        );

        site_manager.delete_from_walrus(all_dynamic_fields).await?;

        // Delete objects on SUI blockchain
        let mut wallet_sui = load_wallet_context(&self.config.general.wallet)?;

        let ptb = SitePtb::new(self.config.package, Identifier::new(SITE_MODULE)?)?;
        let mut ptb = ptb.with_call_arg(&wallet_sui.get_object_ref(site_id).await?.into())?;
        let site = RemoteSiteFactory::new(&wallet_sui.get_client().await?, self.config.package)
            .await?
            .get_from_chain(site_id)
            .await?;

        ptb.destroy(site.resources())?;
        let active_address = wallet_sui.active_address()?;
        let gas_coin = wallet_sui
            .gas_for_owner_budget(active_address, self.config.gas_budget(), BTreeSet::new())
            .await?
            .1
            .object_ref();

        sign_and_send_ptb(
            active_address,
            &wallet_sui,
            ptb.finish(),
            gas_coin,
            self.config.gas_budget(),
        )
        .await?;

        Ok(())
    }
}

impl SiteEditor<EditOptions> {
    /// The directory containing the site sources.
    pub fn directory(&self) -> &Path {
        &self.edit_options.publish_options.directory
    }

    /// Run the editing operations requested.
    pub async fn run(&self) -> Result<()> {
        match self.edit_options.continuous_editing {
            ContinuousEditing::Once => self.run_single_and_print_summary().await?,
            ContinuousEditing::Watch => self.run_continuous().await?,
        }
        Ok(())
    }

    async fn run_single_edit(
        &self,
    ) -> Result<(SuiAddress, SuiTransactionBlockResponse, SiteDataDiffSummary)> {
        if self.edit_options.publish_options.list_directory {
            display::action(format!("Preprocessing: {}", self.directory().display()));
            Preprocessor::preprocess(self.directory())?;
            display::done();
        }

        let wallet = load_wallet_context(&self.config.general.wallet)?;

        let walrus = Walrus::new(
            self.config.walrus_binary(),
            self.config.gas_budget(),
            self.config.general.rpc_url.clone(),
            self.config.general.walrus_config.clone(),
            self.config.general.wallet.clone(),
        );

        let (ws_resources, ws_resources_path) = load_ws_resources(
            &self.edit_options.publish_options.ws_resources,
            self.directory(),
        )?;
        if let Some(path) = ws_resources_path.as_ref() {
            println!(
                "Using the Walrus sites resources file: {}",
                path.to_string_lossy()
            );
        }

        let mut resource_manager = ResourceManager::new(
            walrus.clone(),
            ws_resources,
            ws_resources_path,
            self.edit_options.publish_options.max_concurrent,
        )
        .await?;
        display::action(format!(
            "Parsing the directory {} and locally computing blob IDs",
            self.directory().to_string_lossy()
        ));
        let local_site_data = resource_manager.read_dir(self.directory()).await?;
        display::done();
        tracing::debug!(?local_site_data, "resources loaded from directory");

        let mut site_manager = SiteManager::new(
            self.config.clone(),
            walrus,
            wallet,
            self.edit_options.site_id.clone(),
            self.edit_options.publish_options.epochs,
            self.edit_options.when_upload.clone(),
            self.edit_options.publish_options.permanent.unwrap_or(false),
        )
        .await?;
        let (response, summary) = site_manager.update_site(&local_site_data).await?;
        Ok((site_manager.active_address()?, response, summary))
    }

    async fn run_single_and_print_summary(&self) -> Result<()> {
        let (active_address, response, summary) = self.run_single_edit().await?;
        print_summary(
            &self.config,
            &active_address,
            &self.edit_options.site_id,
            &response,
            &summary,
        )?;
        Ok(())
    }

    async fn run_continuous(&self) -> Result<()> {
        let (tx, rx) = channel();
        let mut watcher = notify::recommended_watcher(move |res| {
            tx.send(res).expect("Error in sending the watch event")
        })?;

        // Add a path to be watched. All files and directories at that path and
        // below will be monitored for changes.
        watcher.watch(self.directory(), RecursiveMode::Recursive)?;

        loop {
            match rx.recv() {
                Ok(event) => {
                    tracing::info!("change detected: {:?}", event);
                    self.run_single_and_print_summary().await?;
                }
                Err(e) => println!("Watch error!: {}", e),
            }
        }
    }
}

fn print_summary(
    config: &Config,
    address: &SuiAddress,
    site_id: &SiteIdentifier,
    response: &SuiTransactionBlockResponse,
    summary: &impl Summarizable,
) -> Result<()> {
    if let Some(SuiTransactionBlockEffects::V1(eff)) = response.effects.as_ref() {
        if let SuiExecutionStatus::Failure { error } = &eff.status {
            return Err(anyhow!(
                "error while processing the Sui transaction: {}",
                error
            ));
        }
    }

    display::header("Execution completed");
    println!("{}\n", summary.to_summary());
    let object_id = match site_id {
        SiteIdentifier::ExistingSite(id) => {
            println!("Site object ID: {}", id);
            *id
        }
        SiteIdentifier::NewSite(name) => {
            let id = get_site_id_from_response(
                *address,
                response
                    .effects
                    .as_ref()
                    .ok_or(anyhow::anyhow!("response did not contain effects"))?,
            )?;
            println!("Created new site: {}\nNew site object ID: {}", name, id);
            id
        }
    };

    println!(
        "Browse the resulting site at: https://{}.{}",
        id_to_base36(&object_id)?,
        config.portal
    );
    Ok(())
}

/// Gets the configuration from the provided file, or looks in the default directory.
fn load_ws_resources(
    path: &Option<PathBuf>,
    site_dir: &Path,
) -> Result<(Option<WSResources>, Option<PathBuf>)> {
    let default_paths = vec![site_dir.join(DEFAULT_WS_RESOURCES_FILE)];
    let path = path_or_defaults_if_exist(path, &default_paths);
    Ok((path.as_ref().map(WSResources::read).transpose()?, path))
}
