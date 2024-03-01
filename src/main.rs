use anyhow::Result;
use emoji::named::*;
use poise::serenity_prelude::*;
use serenity::futures::StreamExt;
use std::fs::read_to_string;
use std::io::Write;
type Context<'a> = poise::Context<'a, (), anyhow::Error>;
#[poise::command(slash_command)]
pub async fn users(c: Context<'_>) -> Result<()> {
    c.defer().await?;
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
        let d = (x.timestamp() as usize - 1420070400) / (60 * 60 * 24);
        min = min.min(d);
        max = max.max(d);
        data[d] += 1;
    }
    let mut f = std::fs::File::create("1.dat").unwrap();
    let mut sum = 0;
    for (i, &d) in data[min..max].iter().enumerate() {
        sum += d;
        let t =
            Timestamp::from_unix_timestamp(((i + min) as i64 * (60 * 60 * 24) as i64) + 1420070400)
                .unwrap();
        writeln!(
            &mut f,
            r"{},{sum}",
            if t.format("%d").to_string() == "01" {
                t.format("%m/%d/%y").to_string()
            } else {
                "".to_string()
            }
        )
        .unwrap();
    }
    assert!(std::process::Command::new("gnuplot")
        .arg("x.plot")
        .spawn()
        .unwrap()
        .wait()
        .unwrap()
        .success());
    assert!(std::process::Command::new("inkscape")
        .arg("--export-filename=data.png")
        .arg("data.svg")
        .spawn()
        .unwrap()
        .wait()
        .unwrap()
        .success());
    poise::send_reply(
        c,
        poise::CreateReply::default().attachment(CreateAttachment::path("data.png").await.unwrap()),
    )
    .await?;
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
