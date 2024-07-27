use std::collections::HashMap;

use anyhow::Context;
use unicode_width::UnicodeWidthStr;

use crate::data::Data;

pub fn authors(data: &mut Data, hash: Option<String>) -> anyhow::Result<()> {
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

    let authors = data.load_authors()?;

    let mut count = HashMap::<String, u64>::new();
    for file in blame.0.values() {
        for (hash, amount) in file {
            let info = data.load_commit(hash.clone())?;
            let author = authors.get(&info.author_mail);
            *count.entry(author).or_default() += amount;
        }
    }

    let mut count = count.into_iter().map(|(a, n)| (n, a)).collect::<Vec<_>>();
    count.sort_unstable();

    for (n, a) in count {
        let n = format!("{n}");
        let space = (78 - a.width() - n.width()).max(1);
        println!("{a} {} {n}", ".".repeat(space));
    }

    Ok(())
}
