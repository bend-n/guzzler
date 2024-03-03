use anyhow::Result;
use chrono::{DateTime, Datelike, TimeZone, Utc};
use emoji::named::*;
use poise::serenity_prelude::*;
use serenity::futures::StreamExt;
use std::fs::read_to_string;
use std::io::Write;
use std::ops::{Deref, Sub};
use std::sync::Arc;
use tokio::sync::Mutex;
type Context<'a> = poise::Context<'a, (), anyhow::Error>;

#[derive(poise::ChoiceParameter, Default, Copy, Clone, Debug)]
enum Period {
    #[name = "all time"]
    #[default]
    AllTime,
    #[name = "year"]
    LastYear,
    #[name = "month"]
    LastMonth,
    #[name = "week"]
    LastWeek,
}

#[derive(Copy, Clone, Debug)]
struct Days(usize);

impl Deref for Days {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Sub<usize> for Days {
    type Output = Days;

    fn sub(self, rhs: usize) -> Self::Output {
        Self(self.0 - rhs)
    }
}

impl Days {
    fn t(self) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(((*self as i64 * (60 * 60 * 24) as i64) + 1420070400) * 1000)
            .single()
            .unwrap()
    }

    fn now() -> Self {
        Self::from(Utc::now().timestamp() as usize)
    }
}

impl From<usize> for Days {
    fn from(value: usize) -> Self {
        Self((value - 1420070400) / (60 * 60 * 24))
    }
}

impl Period {
    fn from(self) -> Days {
        match self {
            Self::AllTime => Days(0),
            Self::LastYear => Days::now() - 365,
            Self::LastMonth => Days::now() - 30,
            Self::LastWeek => Days::now() - 7,
        }
    }
    fn tic(self, t: Days) -> String {
        match self {
            Period::AllTime if t.t().day() == 1 => t.t().format("%m/%y/%d").to_string(),
            Period::LastYear if t.t().day() == 1 => t.t().format("%B %d").to_string(),
            Period::LastMonth if t.t().day() % 7 == 0 => t.t().format("%B %d").to_string(),
            Period::LastWeek => t.t().format("%A").to_string(),
            _ => "".to_string(),
        }
    }
}

#[poise::command(slash_command)]
/// graph of messages over the last 4 days
pub async fn msgs(c: Context<'_>) -> Result<()> {
    c.defer().await?;
    let (min, cs) = {
        let Some(g) = c.guild() else {
            _ = c.reply(format!("{CANCEL} need guild"));
            return Ok(());
        };
        (
            Days::from(g.id.created_at().timestamp() as usize),
            g.channels.values().map(|x| x.id).collect::<Vec<_>>(),
        )
    };

    let data: Arc<Mutex<Vec<u16>>> = Arc::new(Mutex::new(vec![0; *Days::now() + 1]));
    let cutoff = Days::now() - 3;
    futures::stream::iter(cs)
        .map(|x| {
            let data = data.clone();
            async move {
                let mut c = std::pin::pin!(x.messages_iter(c));

                while let Some(Ok(x)) = c.next().await {
                    if x.author.bot {
                        continue;
                    }
                    let x = x.timestamp;
                    let d = *Days::from(x.timestamp() as usize);
                    if d < *cutoff {
                        break;
                    }
                    data.lock().await[d] += 1;
                }
            }
        })
        .buffer_unordered(32)
        .for_each(|_| async {})
        .await;
    let min = min.max(*cutoff);
    let i = c.id();
    let data = data.lock().await;
    let mut f = std::fs::File::create(format!("{i}.dat")).unwrap();
    for (i, &d) in data[min..].iter().enumerate() {
        writeln!(&mut f, r"{},{d}", Period::LastWeek.tic(Days(i + min))).unwrap();
    }
    let plot = if cfg!(debug_assertions) {
        std::fs::read_to_string("y.plot").unwrap()
    } else {
        include_str!("../y.plot").to_string()
    };
    let cleanup = tokio::task::spawn_blocking(move || run(plot, &[], i))
        .await
        .unwrap();

    poise::send_reply(
        c,
        poise::CreateReply::default()
            .attachment(CreateAttachment::path(format!("{i}.png")).await.unwrap()),
    )
    .await?;
    tokio::task::spawn_blocking(cleanup).await.unwrap();
    Ok(())
}

fn run(
    mut plot: String,
    templates: &[(&'static str, &dyn std::fmt::Display)],
    i: u64,
) -> impl FnOnce() -> () {
    for (key, value) in templates {
        plot = plot.replace(&format!("{{{key}}}"), &value.to_string())
    }
    let plot = plot.replace("{id}", &i.to_string());
    let path = format!("{i}.plot");
    std::fs::File::create_new(&path)
        .unwrap()
        .write_all(plot.as_bytes())
        .unwrap();

    assert!(std::process::Command::new("gnuplot")
        .arg(&path)
        .spawn()
        .unwrap()
        .wait()
        .unwrap()
        .success());
    assert!(std::process::Command::new("inkscape")
        .arg(format!("--export-filename={i}.png"))
        .arg(format!("{i}.svg"))
        .spawn()
        .unwrap()
        .wait()
        .unwrap()
        .success());
    move || {
        std::fs::remove_file(format!("{i}.dat")).unwrap();
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(format!("{i}.svg")).unwrap();
        std::fs::remove_file(format!("{i}.png")).unwrap();
    }
}

#[poise::command(slash_command)]
/// graph of users over time
pub async fn users(
    c: Context<'_>,
    #[description = "time period to graph (defaults to all time)"] period: Option<Period>,
) -> Result<()> {
    c.defer().await?;
    let p = period.unwrap_or_default();
    let g = {
        let Some(g) = c.guild() else {
            _ = c.reply(format!("{CANCEL} need guild"));
            return Ok(());
        };
        g.id
    };
    let mut s = std::pin::pin!(g.members_iter(c));
    let mut data: Vec<u16> = vec![0; *Days::now() + 1];
    let min = Days::from(g.created_at().timestamp() as usize);
    let mut max = 0;
    while let Some(x) = s.next().await {
        let x = x?.joined_at.unwrap();
        let d = *Days::from(x.timestamp() as usize);
        max = max.max(d);
        data[d] += 1;
    }
    let min = min.max(*p.from());
    let i = c.id();

    let mut f = std::fs::File::create(format!("{i}.dat")).unwrap();
    let mut sum = data[..min].iter().map(|&x| x as u64).sum::<u64>();
    let floor = sum;
    for (i, &d) in data[min..max].iter().enumerate() {
        sum += d as u64;
        writeln!(&mut f, r"{},{sum}", p.tic(Days(i + min))).unwrap();
    }
    let plot = if cfg!(debug_assertions) {
        std::fs::read_to_string("x.plot").unwrap()
    } else {
        include_str!("../x.plot").to_string()
    };

    let cleanup = tokio::task::spawn_blocking(move || {
        run(
            plot,
            &[(
                "floor",
                &match p {
                    Period::AllTime => "".to_string(),
                    _ => floor.to_string(),
                },
            )],
            i,
        )
    })
    .await
    .unwrap();

    poise::send_reply(
        c,
        poise::CreateReply::default()
            .attachment(CreateAttachment::path(format!("{i}.png")).await.unwrap()),
    )
    .await?;
    tokio::task::spawn_blocking(cleanup).await.unwrap();
    Ok(())
}

#[tokio::main]
async fn main() {
    let tok =
        std::env::var("TOKEN").unwrap_or_else(|_| read_to_string("token").expect("wher token"));
    let f = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![users(), msgs()],
            ..Default::default()
        })
        .setup(|ctx, _ready, f| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &f.options().commands).await?;
                println!("registered");
                Ok(())
            })
        })
        .build();
    ClientBuilder::new(
        tok,
        GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT,
    )
    .framework(f)
    .await
    .unwrap()
    .start()
    .await
    .unwrap();
}
