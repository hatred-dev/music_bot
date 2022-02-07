use ini::Ini;
use openweathermap::{blocking::weather as open_weather, CurrentWeather};
use serenity::{
    async_trait,
    client::Context,
    client::{Client, EventHandler},
    framework::{
        standard::{
            macros::{command, group},
            Args, CommandResult,
        },
        StandardFramework,
    },
    http::Http,
    model::{channel::Message, gateway::Ready, id::ChannelId},
    prelude::{Mutex, TypeMapKey},
    Result as SerenityResult,
};
use songbird::{input, Call, SerenityInit};
use std::sync::Arc;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[group]
#[commands(
    list, deafen, join, leave, mute, play, ping, undeafen, unmute, stop, weather, volume, skip,
    pause, resume
)]
struct General;

struct Config {
    token: String,
    prefix: String,
    openweather: OpenWeather,
}

#[derive(Clone)]
struct OpenWeather {
    token: Option<String>,
    location: Option<String>,
    system: Option<String>,
}
impl TypeMapKey for OpenWeather {
    type Value = OpenWeather;
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    //load configuration file
    let conf = load_config("config.ini");
    let token = conf.token;
    let framework = StandardFramework::new()
        .configure(|c| c.prefix(conf.prefix))
        .group(&GENERAL_GROUP);

    let mut client = Client::builder(&token)
        .event_handler(Handler)
        .framework(framework)
        .register_songbird()
        .await
        .expect("Err creating client");
    {
        let mut data = client.data.write().await;
        data.insert::<OpenWeather>(conf.openweather);
    }

    let _ = client
        .start()
        .await
        .map_err(|why| println!("Client ended: {:?}", why));
}

async fn send_weather_message(id: ChannelId, http: &Arc<Http>, current: &CurrentWeather) {
    check_msg(id.send_message(http, |m| {
        m.embed(|e| {
            e.author(|a| {
                a.name("HATRED");
                a.icon_url("https://cdn.discordapp.com/avatars/223800736809615360/59665bbb7ae61ddaa066b6586e9d18b5.png")
            });
            e.title(format!("Today's weather in **{}**", current.name.as_str()));
            e.thumbnail(format!("https://openweathermap.org/img/wn/{}@4x.png", current.weather[0].icon));
            e.colour(15105570);
            e.description(format!("Weather: **{}**\n\
                        Temperature: **{}°C**\n\
                        Feels like: **{}°C**\n\
                        Humidity: **{}%**\n\
                        Wind: **{}m/s**\n\
                        Description: **{}**",
                                  current.weather[0].main.as_str().to_lowercase(),
                                  current.main.temp,
                                  current.main.feels_like,
                                  current.main.humidity,
                                  current.wind.speed,
                                  current.weather[0].description.as_str()))
        });
        m
    }).await);
}

#[command]
#[only_in(guilds)]
async fn weather(ctx: &Context, msg: &Message) -> CommandResult {
    let open_weather_conf = {
        let data_read = ctx.data.read().await;
        data_read
            .get::<OpenWeather>()
            .expect("Missing openweather configuration")
            .clone()
    };
    if open_weather_conf.token.is_none() {
        check_msg(
            msg.channel_id
                .say(
                    &ctx.http,
                    "You did not specify openweather api key in configuration file",
                )
                .await,
        );
        return Ok(());
    }
    let open_weather_obj = &open_weather(
        open_weather_conf.location.as_ref().unwrap(),
        open_weather_conf.system.as_ref().unwrap(),
        "en",
        open_weather_conf.token.as_ref().unwrap(),
    )
    .unwrap();
    send_weather_message(msg.channel_id, &ctx.http, open_weather_obj).await;
    Ok(())
}

#[command]
#[only_in(guilds)]
async fn pause(ctx: &Context, msg: &Message) -> CommandResult {
    if let Some(handler_lock) = acquire_lock_and_check_voice(ctx, msg).await {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        match queue.pause() {
            Ok(_) => {}
            Err(e) => check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed to pause track: {}", e))
                    .await,
            ),
        };
    };
    Ok(())
}

#[command]
#[only_in(guilds)]
async fn resume(ctx: &Context, msg: &Message) -> CommandResult {
    if let Some(handler_lock) = acquire_lock_and_check_voice(ctx, msg).await {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        match queue.resume() {
            Ok(_) => {}
            Err(e) => check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed to resume track: {}", e))
                    .await,
            ),
        };
    };
    Ok(())
}

#[command]
#[only_in(guilds)]
async fn deafen(ctx: &Context, msg: &Message) -> CommandResult {
    let handler_lock = acquire_lock_and_check_voice(ctx, msg).await.unwrap();
    let mut handler = handler_lock.lock().await;
    if handler.is_deaf() {
        check_msg(msg.channel_id.say(&ctx.http, "Already deafened").await);
    } else {
        if let Err(e) = handler.deafen(true).await {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await,
            );
        }
        check_msg(msg.channel_id.say(&ctx.http, "Deafened").await);
    }
    Ok(())
}

#[command]
#[only_in(guilds)]
async fn join(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let channel_id = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id);

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            check_msg(msg.reply(ctx, "Not in a voice channel").await);
            return Ok(());
        }
    };

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let _handler = manager.join(guild_id, connect_to).await;
    Ok(())
}

#[command]
#[only_in(guilds)]
async fn skip(ctx: &Context, msg: &Message) -> CommandResult {
    let handler_lock = acquire_lock_and_check_voice(ctx, msg).await.unwrap();
    let handler = handler_lock.lock().await;
    let queue = handler.queue();
    match queue.skip() {
        Ok(_) => {}
        Err(e) => check_msg(
            msg.channel_id
                .say(&ctx.http, format!("Couldn't skip track: {}", e))
                .await,
        ),
    };
    Ok(())
}

#[command]
#[only_in(guilds)]
async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();
    let has_handler = manager.get(guild_id).is_some();
    if has_handler {
        if let Err(e) = manager.remove(guild_id).await {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await,
            );
        }

        check_msg(msg.channel_id.say(&ctx.http, "Left voice channel").await);
    } else {
        check_msg(msg.reply(ctx, "Not in a voice channel").await);
    }
    Ok(())
}

#[command]
#[only_in(guilds)]
async fn mute(ctx: &Context, msg: &Message) -> CommandResult {
    let handler_lock = acquire_lock_and_check_voice(ctx, msg).await.unwrap();
    let mut handler = handler_lock.lock().await;

    if handler.is_mute() {
        check_msg(msg.channel_id.say(&ctx.http, "Already muted").await);
    } else {
        if let Err(e) = handler.mute(true).await {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await,
            );
        }
        check_msg(msg.channel_id.say(&ctx.http, "Now muted").await);
    }
    Ok(())
}

#[command]
async fn ping(context: &Context, msg: &Message) -> CommandResult {
    check_msg(msg.channel_id.say(&context.http, "Pong!").await);
    Ok(())
}

#[command]
#[only_in(guilds)]
async fn volume(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let limit = 100f32;
    let mut vol = match args.single::<f32>() {
        Ok(volume) => volume,
        Err(_) => {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, "Numeric value from 1 to 100 is needed")
                    .await,
            );
            return Ok(());
        }
    };
    if vol > limit {
        vol = limit;
    }
    vol /= limit;
    if let Some(handler_lock) = acquire_lock_and_check_voice(ctx, msg).await {
        let handler = handler_lock.lock().await;
        let q = handler.queue();
        if let Some(cur) = q.current() {
            match cur.set_volume(vol) {
                Ok(_) => {}
                Err(e) => check_msg(
                    msg.channel_id
                        .say(&ctx.http, format!("Couldn't change volume: {}", e))
                        .await,
                ),
            };
        }
    };
    Ok(())
}

#[command]
#[only_in(guilds)]
async fn play(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let url = match args.single::<String>() {
        Ok(url) => url,
        Err(_) => {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, "Must provide a URL to a video or audio")
                    .await,
            );
            return Ok(());
        }
    };

    if !url.starts_with("http") {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Must provide a valid URL")
                .await,
        );
        return Ok(());
    }

    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;
    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();
    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.try_lock()?;
        let source = match input::ytdl(&url).await {
            Ok(source) => source,
            Err(why) => {
                let error = format!("Error starting source {:?}", why);
                check_msg(msg.channel_id.say(&ctx.http, error).await);
                return Ok(());
            }
        };
        if !handler.queue().current_queue().is_empty() {
            check_msg(
                msg.channel_id
                    .say(
                        &ctx.http,
                        format!(
                            "Song placed in queue and will be played after **{}**",
                            handler
                                .queue()
                                .current()
                                .unwrap()
                                .metadata()
                                .title
                                .as_ref()
                                .unwrap()
                                .as_str()
                        ),
                    )
                    .await,
            );
        } else {
            check_msg(
                msg.channel_id
                    .say(
                        &ctx.http,
                        format!(
                            "Playing: **{}**",
                            &source
                                .metadata
                                .title
                                .as_deref()
                                .unwrap_or("Unable to get title")
                        ),
                    )
                    .await,
            );
        }
        handler.enqueue_source(source);
    } else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Not in a voice channel to play in")
                .await,
        );
    }
    Ok(())
}

#[command]
#[only_in(guilds)]
async fn list(ctx: &Context, msg: &Message) -> CommandResult {
    if let Some(handler_lock) = acquire_lock_and_check_voice(ctx, msg).await {
        let handler = handler_lock.lock().await;
        let mut song_list = String::new();
        for (pos, track) in handler.queue().current_queue().iter().enumerate() {
            song_list.push_str(
                format!(
                    "{}. **{}**\n",
                    (pos + 1).to_string().as_str(),
                    track.metadata().title.as_ref().unwrap().as_str()
                )
                .as_str(),
            );
        }
        check_msg(if song_list.is_empty() {
            msg.channel_id
                .say(&ctx.http, "Current song list is empty :(")
                .await
        } else {
            msg.channel_id
                .say(&ctx.http, format!("Current song list:\n{}", song_list))
                .await
        });
    }
    Ok(())
}

#[command]
#[only_in(guilds)]
async fn stop(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;
    let manager = songbird::get(ctx)
        .await
        .expect("Voice client placed at initialisation")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        handler.queue().stop();
        check_msg(msg.channel_id.say(&ctx.http, "Stopped").await);
    } else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Not in a voice channel to stop")
                .await,
        );
    }
    Ok(())
}

#[command]
#[only_in(guilds)]
async fn undeafen(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        if let Err(e) = handler.deafen(false).await {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await,
            );
        }

        check_msg(msg.channel_id.say(&ctx.http, "Undeafened").await);
    } else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Not in a voice channel to undeafen in")
                .await,
        );
    }
    Ok(())
}

#[command]
#[only_in(guilds)]
async fn unmute(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        if let Err(e) = handler.mute(false).await {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await,
            );
        }
        check_msg(msg.channel_id.say(&ctx.http, "Unmuted").await);
    } else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Not in a voice channel to unmute in")
                .await,
        );
    }
    Ok(())
}

fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}

async fn acquire_lock_and_check_voice(ctx: &Context, msg: &Message) -> Option<Arc<Mutex<Call>>> {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;
    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();
    return match manager.get(guild_id) {
        Some(handler) => Some(handler),
        None => {
            check_msg(msg.reply(ctx, "Not in a voice channel").await);
            None
        }
    };
}

fn load_config(file: &str) -> Config {
    let conf = Ini::load_from_file(file).unwrap();
    let discord_section = conf.section(Some("discord")).unwrap();
    let token = discord_section.get("token").unwrap().to_string();
    let prefix = discord_section.get("prefix").unwrap().to_string();
    let weather_section = conf.section(Some("openweather")).unwrap();
    let weather_token = weather_section.get("openweather_token").map(str::to_string);
    let mut location = String::new();
    let mut system = String::new();
    if weather_token.is_some() {
        location = weather_section
            .get("location")
            .expect("Missing location.")
            .to_string();
        system = weather_section
            .get("measurement_system")
            .expect("Missing measurement system.")
            .to_string();
    }

    Config {
        token,
        prefix,
        openweather: OpenWeather {
            token: weather_token,
            location: Some(location),
            system: Some(system),
        },
    }
}
