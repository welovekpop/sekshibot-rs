# SekshiBot
But it's Rust.

This is a chat moderation-and-more bot for the [WLK community](https://wlk.yt).

## Motivation
The old [SekshiBot](https://github.com/welovekpop/sekshibot) is based on Node.js and developed a whole bot framework. When work on that started, we were on [plug.dj](https://plug.dj) and growing rapidly. We needed to do user and chat logging in the bot. Later we moved to Slack and then üWave, so SekshiBot grew multi-backend support. Now, our needs have changed. The Slack is mostly obsolete, as is user and chat logging since we can access that directly in üWave. The resource consumption of MongoDB + Node.js basically requires a full-blown VPS to run the bot at $5/month. The intent with this project is to scale it down, remove flexibility where it is not needed, and run it on an already-existing server (basically for free).

## License
[GPL-3.0](./LICENSE.md)
