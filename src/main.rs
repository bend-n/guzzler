use anyhow::Result;
use chrono::{DateTime, Datelike, TimeZone, Utc};
use emoji::named::*;
use poise::serenity_prelude::*;
use serenity::futures::StreamExt;
use std::fs::read_to_string;
use std::io::Write;
use std::ops::{Deref, Sub};
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

    fn tic(self, t: Days) -> bool {
        match self {
            Self::LastYear | Self::AllTime => t.t().day() == 1,
            Self::LastMonth => t.t().day() % 7 == 0,
            Self::LastWeek => true,
        }
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
    let mut data: Vec<u16> = vec![0; 365 * 12];
    let mut min = 365 * 12;
    let mut max = 0;
    while let Some(x) = s.next().await {
        let x = x?.joined_at.unwrap();
        let d = *Days::from(x.timestamp() as usize);
        min = min.min(d);
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
        writeln!(
            &mut f,
            r"{},{sum}",
            if p.tic(Days(i + min)) {
                Days(i + min).t().format("%m/%d/%y").to_string()
            } else {
                "".to_string()
            }
        )
        .unwrap();
    }
    let plot = if cfg!(debug_assertions) {
        std::fs::read_to_string("x.plot").unwrap()
    } else {
        include_str!("../x.plot").to_string()
    };

    let plot = plot
        .replace(
            "{floor}",
            &match p {
                Period::AllTime => "".to_string(),
                _ => floor.to_string(),
            },
        )
        .replace("{id}", &i.to_string());
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
    poise::send_reply(
        c,
        poise::CreateReply::default()
            .attachment(CreateAttachment::path(format!("{i}.png")).await.unwrap()),
    )
    .await?;
    tokio::task::spawn_blocking(move || {
        std::fs::remove_file(format!("{i}.dat")).unwrap();
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(format!("{i}.svg")).unwrap();
        std::fs::remove_file(format!("{i}.png")).unwrap();
    })
    .await
    .unwrap();
    Ok(())
}

#[tokio::main]
async fn main() {
    let tok =
        std::env::var("TOKEN").unwrap_or_else(|_| read_to_string("token").expect("wher token"));
    let f = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![users()],
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
