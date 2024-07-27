use std::collections::HashMap;

use anyhow::Context;
use unicode_width::UnicodeWidthStr;

use crate::data::Data;

pub fn years(data: &mut Data, hash: Option<String>) -> anyhow::Result<()> {
    let hash = match hash {
        Some(hash) => hash,
        None => data
            .load_log()?
            .first()
            .cloned()
            .ok_or(anyhow::anyhow!("found no viable hash"))?,
    };

    let blame = data
        .load_blame(&hash)
        .context(format!("found no blame for {hash}"))?;

    let mut count = HashMap::<i16, u64>::new();
    for file in blame.0.values() {
        for (hash, amount) in file {
            let info = data.load_commit(hash.clone())?;
            *count.entry(info.author_time.year()).or_default() += amount;
        }
    }

    let mut count = count.into_iter().collect::<Vec<_>>();
    count.sort_unstable();

    for (y, n) in count {
        let n = format!("{n}");
        let y = format!("{y:4}");
        let space = (18 - y.width() - n.width()).max(1);
        println!("{y} {} {n}", ".".repeat(space));
    }

    Ok(())
}
