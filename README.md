# SekshiBot
> But it's Rust.

![SekshiBot](./assets/sekshibot.png)

<small>Image by [@pledi](https://github.com/pledi).</small>

This is a chat moderation-and-more bot for the [WLK community](https://wlk.yt).

## Motivation
The old [SekshiBot](https://github.com/welovekpop/sekshibot) is based on Node.js and developed a whole bot framework. When work on that started, we were on [plug.dj](https://plug.dj) and growing rapidly. We needed to do user and chat logging in the bot. Later we moved to Slack and then üWave, so SekshiBot grew multi-backend support. Now, our needs have changed. The Slack is mostly obsolete, as is user and chat logging since we can access that directly in üWave. The resource consumption of MongoDB + Node.js basically requires a full-blown VPS to run the bot at $5/month. The intent with this project is to scale it down, remove flexibility where it is not needed, and run it on an already-existing server (basically for free).

## Running it
Some environment variables are required:
| Name | Description |
|-|-|
| `SEKSHIBOT_EMAIL` | The email address for the bot's account on üWave |
| `SEKSHIBOT_PASSWORD` | The password for the bot's account on üWave |
| `NEOCITIES_USERNAME` | Neocities username, to publish the !emotes overview page to |
| `NEOCITIES_PASSWORD` | Neocities password |

And command-line parameters:
| Name | Description |
|-|-|
| `--api-url` | URL to the üWave HTTP API |
| `--socket-url` | URL to the üWave WebSocket API |

The bot will exit with code 75 if its login expired, or exit with another nonzero exit code if it crashes for other reasons.
You can autorestart it with systemd or a similar system. If someone does `!exit` in chat, the bot exits with code 0, and it should probably not restart automatically.

## Commands

| Name | Description |
|-|-|
| `!e [emote]` | Display a reaction gif. |
| `!addemote [emote] [url]` | Add a new reaction gif. |
| `!emotes` | Send a link to a page with all the reaction gifs. |
| `!skiplist add [media] "[reason]" | Add a song to the autoskip list. `[media]` is formatted as sourcetype:id, eg. `youtube:123456abc` |

## License
[GPL-3.0](./LICENSE.md)
