use clap::{crate_authors, Clap};
use keyring::Keyring;
use num_format::{Locale, ToFormattedString};
use serenity::client::ClientBuilder;
use serenity::framework::standard::StandardFramework;
use serenity::http::AttachmentType;
use serenity::model::id::ChannelId;
use std::env;
use std::path::PathBuf;
use std::str::FromStr;
use toml_edit::value;

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

#[derive(Debug, serde::Deserialize)]
struct GlobalConfig {
    channel_id: u64,
    name: String,
    pp_url: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Video {
    title: String,
    views: u64,

    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    published: bool,
}

#[tokio::main]
async fn main() {
    let keyring = Keyring::new("fdahm", "");

    match Args::parse() {
        Args::Login(LoginArgs { token }) => {
            keyring.set_password(&*token).expect("cannot set password");
            println!("Token successfully saved");
        }
        Args::List(ListArgs { all }) => unimplemented!(),
        Args::New(NewArgs { slug, title, views }) => {
            let cd = env::current_dir().unwrap();
            let dir = cd.join(&*slug);

            std::fs::create_dir(&*dir).unwrap();

            let initial = Video {
                title: title.unwrap_or(slug),
                views: views.unwrap_or_default(),
                published: false,
            };

            std::fs::write(dir.join("video.toml"), toml::to_string(&initial).unwrap()).unwrap();
        }
        Args::Publish(PublishArgs { name, force }) => {
            let cd = env::current_dir().unwrap();
            let channel: GlobalConfig = toml::from_str(
                &*std::fs::read_to_string(cd.join("channel.toml")).expect("channel.toml not found"),
            )
            .expect("malformed channel.toml");

            let dir = cd.join(name);
            let thumbnail = dir.join("thumbnail.jpg");
            let video = read_video(&dir);

            if video.published && !force {
                eprintln!("This video was apparently already published. Use --force to force.");
                return;
            }

            publish(keyring.get_password().unwrap(), (channel, video, thumbnail)).await;

            // Publish
            {
                let path = dir.join("video.toml");
                let file = std::fs::read_to_string(&path).expect("video.toml not found");
                let mut doc = toml_edit::Document::from_str(&*file).unwrap();

                doc.root["published"] = value(true);
                // doc.root["published_date"] = value(Datetime::OffsetDateTime(Utc::now())) TODO

                std::fs::write(&path, doc.to_string()).expect("Couldn't write updated video.toml");
            }

            println!("Publication successful");
        }
    }
}

fn read_video(path: &PathBuf) -> Video {
    toml::from_str(
        &*std::fs::read_to_string(path.join("video.toml")).expect("video.toml not found"),
    )
    .expect("malformed video.toml")
}

/*fn list() -> std::io::Result<Vec<(Video, bool)>> {
    use std::fs::*;

    let cd = env::current_dir()?;

    let default: PartialVideo = read_to_string(cd.join("default.toml"))
        .map(|content| toml::from_str(&*content).expect("Malformed default.toml"))
        .unwrap_or_default();

    let dirs = read_dir(cd)?
        .map(|e| e.unwrap())
        .filter(|e| e.metadata().unwrap().is_dir())
        .map(|e| e.path());

    let videos = dirs
        .map(|d| {
            let video = read_to_string(d.join("video.toml")).expect("Cannot read video.toml");
            let video: PartialVideo = toml::from_str(&*video).expect("Malformed video.toml");

            let video = video
                .inherit(&default)
                .expect("Missing properties after merging default.toml and video.toml");

            let published = d.join(".published").exists();

            (video, published)
        })
        .collect();

    Ok(videos)
}*/

async fn publish(token: String, video: (GlobalConfig, Video, PathBuf)) {
    let client = ClientBuilder::new(token)
        // .event_handler(Events(sender))
        .framework(StandardFramework::new())
        .await
        .expect("error creating client");

    let (global, video, thumbnail) = video;
    let video_views = video.views;
    let video_title = video.title;

    // Hackiest hack of 2020: I upload the image as a message in a channel (doesn't matter which)
    // and get its URL, to use Discord as an image hosting service.
    // I might replace this with something a bit cleaner but it works, so...
    let url = {
        let mut image_message = ChannelId(762636492421857300)
            .send_message(client.cache_and_http.http.as_ref(), |m| {
                m.add_file(AttachmentType::Path(thumbnail.as_ref()))
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
}
