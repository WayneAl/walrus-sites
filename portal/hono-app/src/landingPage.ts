// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { html } from "hono/html"

export const landingPage = html`
<!DOCTYPE html>
<html lang="en">
    <head>
        <meta charset="UTF-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        <title>Walrus Sites</title>
        <link rel="icon" href="static/images/favicon.ico" type="image/x-icon">
        <link rel="apple-touch-icon" sizes="180x180" href="static/images/apple-touch-icon.png">
        <link rel="icon" sizes="32x32" href="static/images/favicon-32x32.png" type="image/png">
        <link rel="icon" sizes="16x16" href="static/images/favicon-16x16.png" type="image/png">
        <link rel="stylesheet" href="static/concrete.css" />
        <link rel="stylesheet" href="static/normalize.css" />
    </head>
    <body>
        <main>
            <div class="main-div">
                <div class="content-div">
                    <div class="AlumniSans custom-header">
                        Walrus Sites

                    </div>
                    <div class="custom-p InterTightMedium center" style="gap: 10px;">
                        <div>A Walrus site is a website that exists entirely on
                        The Walrus Decentralized Store, eliminating the need for
                        traditional web infrastructure.
                            Walrus sites load their content from objects on Walrus,
                            which include all the resources required to render the site.</div>
                            <div>To access a Walrus site, all you need is its Sui Object ID or
                            SuiNS name.
                            There's no need for a wallet; these sites are accessible to everyone.
                            Walrus sites take full advantage of the decentralization provided by
                            The Walrus Store, offering  low storage costs compared to traditional
                            web2 solutions
                            and greater robustness vs web3 alternatives.</div>
                        <div>With Walrus sites, Dapps can achieve full decentralization, extending
                        beyond smart contracts. Dapps on Sui, Ethereum, Solana,
                            and other L1 and L2 networks can utilize Walrus Sites as a robust
                            and highly available
                            alternative to their existing cloud hosting solutions.</div>
                        <div>Walrus is secured by <a style="color: #696969;"
                            href="https://sui.io/" target="_blank">Sui</a>, a horizontally
                            scalable Byzantine fault-tolerant and proof-of-stake blockchain.
                            With Walrus, data is sharded and efficiently replicated across Walrus
                            nodes globally.
                            This marks a significant step forward in the evolution of fully
                            decentralized applications.</div>
                            </div>
                            <div class="start">
                        <a class="docs InterTightBold"
                        href="https://docs.walrus.site/" target="_blank">
                            Read the Docs
                        </a>
                    </div>
                </div>
                <footer class="InterTightBold">
                    <div>
                        Copyright 2024 © Mysten Labs, Inc.
                    </div>
                    <div class="terms-policies">
                        <a href="https://docs.walrus.site/walrus-sites/tos.html"
                        target="_blank" style="text-decoration: none; color: #696969;">
                        Terms of Service
                        </a>
                        <a href="https://docs.walrus.site/walrus-sites/privacy.html"
                        target="_blank" style="text-decoration: none; color: #696969;">
                            Privacy Policy
                        </a>
                    </div>
                </footer>
            </div>
        </main>
    </body>
</html>

`
