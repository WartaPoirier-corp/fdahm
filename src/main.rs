mod fdahm_result;
mod fdahm_storage;

use crate::fdahm_result::{FdahmError, FdahmResult};
use crate::fdahm_storage::{FdahmDirectory, GlobalConfig, Video, VideoMeta};
use clap::{crate_authors, Clap};
use keyring::Keyring;
use num_format::{Locale, ToFormattedString};
use serenity::client::ClientBuilder;
use serenity::framework::standard::StandardFramework;
use serenity::http::AttachmentType;
use serenity::model::id::ChannelId;
use std::env;
use std::path::PathBuf;

#[derive(Clap)]
#[clap(author = crate_authors!())]
enum Args {
    /// Store a discord token in the OS's keyring
    Login(LoginArgs),

    /// List "videos" in the current directory
    List(ListArgs),

    /// Create a new video directory with a default `video.toml`
    New(NewArgs),

    /// Publish a "video" to Discord and mark it as published
    Publish(PublishArgs),
}

#[derive(Clap)]
struct LoginArgs {
    token: String,
}

#[derive(Clap)]
struct ListArgs {
    /// Include already published "videos"
    #[clap(short, long)]
    all: bool,
}

#[derive(Clap)]
struct NewArgs {
    /// Name of the directory
    slug: String,

    /// Title of the video
    title: Option<String>,

    /// View count
    #[clap(short, long)]
    views: Option<u64>,
}

#[derive(Clap)]
struct PublishArgs {
    name: String,

    /// Publish a video even if it is marked as already published
    #[clap(short, long)]
    force: bool,
}

#[tokio::main]
async fn main() {
    std::process::exit(match main_result().await {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("error: {:?}", e);
            1
        }
    })
}

async fn main_result() -> FdahmResult<()> {
    let keyring = Keyring::new("fdahm", "");

    let cd = &std::env::current_dir().unwrap();
    let fdahm = FdahmDirectory::new(cd)?;

    match Args::parse() {
        Args::Login(LoginArgs { token }) => {
            keyring.set_password(&*token).expect("cannot set password");
            println!("Token successfully saved");
            Ok(())
        }
        Args::List(ListArgs { all }) => {
            let mut videos = fdahm.list_videos()?;

            if !all {
                videos.retain(|v| !v.meta.published);
            }

            for video in videos {
                let opt_strikethrough = if video.meta.published { "\x1b[9m" } else { "" };
                println!("    {}{}\x1b[0m", opt_strikethrough, video.id)
            }

            Ok(())
        }
        Args::New(NewArgs { slug, title, views }) => {
            let cd = env::current_dir().unwrap();
            let dir = cd.join(&*slug);

            std::fs::create_dir(&*dir).unwrap();

            let initial = VideoMeta {
                title: title.unwrap_or(slug),
                views: views.unwrap_or_default(),
                published: false,
            };

            std::fs::write(dir.join("video.toml"), toml::to_string(&initial).unwrap()).unwrap();

            Ok(())
        }
        Args::Publish(PublishArgs { name, force }) => {
            let config = fdahm.global_config()?;
            let video = fdahm.get_video_by_id(name)?;

            if video.meta.published && !force {
                eprintln!("This video was apparently already published. Use --force to force.");
                return Err(FdahmError::AlreadyPublished);
            }

            let thumbnail = fdahm.get_thumbnail(&video)?;
            publish(
                keyring.get_password().unwrap(),
                (config, &video, &thumbnail),
            )
            .await?;

            fdahm.mark_published(&video)?;

            println!("Publication successful");

            Ok(())
        }
    }
}

async fn publish(token: String, video: (GlobalConfig, &Video, &PathBuf)) -> FdahmResult<()> {
    let client = ClientBuilder::new(token)
        // .event_handler(Events(sender))
        .framework(StandardFramework::new())
        .await
        .expect("error creating client");

    let (global, video, thumbnail) = video;
    let video_views = video.meta.views;
    let video_title = &video.meta.title;

    // Hackiest hack of 2020: I upload the image as a message in a channel (doesn't matter which)
    // and get its URL, to use Discord as an image hosting service.
    // I might replace this with something a bit cleaner but it works, so...
    let url = {
        let mut image_message = ChannelId(762636492421857300)
            .send_message(client.cache_and_http.http.as_ref(), |m| {
                m.add_file(AttachmentType::Path(thumbnail))
            })
            .await
            .unwrap();

        image_message.attachments.remove(0).url
    };

    ChannelId(global.channel_id)
        .send_message(client.cache_and_http.http.as_ref(), |m| {
            m.embed(|e| {
                e.color(0xFF0000)
                    .author(|a| {
                        a.name(global.name)
                            .url("https://github.com/WartaPoirier-corp/fdahm")
                            .icon_url(global.pp_url)
                    })
                    .title(video_title)
                    .footer(|f| {
                        f.icon_url("https://www.youtube.com/s/desktop/b4620429/img/favicon_48.png")
                            .text(format!(
                                "{} vues",
                                video_views.to_formatted_string(&Locale::fr)
                            ))
                    })
                    .timestamp(chrono::Utc::now().to_rfc3339())
                    .image(url)
            })
        })
        .await
        .unwrap();

    Ok(())
}
