use std::{collections::HashMap, path::Path};

use crate::data::Data;

pub fn authors(
    datafile: &Path,
    renames: &HashMap<String, String>,
    hash: Option<String>,
) -> anyhow::Result<()> {
    let data = Data::load(datafile)?;

    let Some(hash) = hash.as_ref().or(data.log.first()) else {
        anyhow::bail!("found no viable hash");
    };

    let Some(blame) = data.blames.get(hash) else {
        anyhow::bail!("found no blame for {hash}");
    };

    let mut count = HashMap::<String, u64>::new();
    for file in blame.0.values() {
        for (commit, amount) in file {
            let Some(info) = data.commits.get(commit) else {
                anyhow::bail!("found no info for {commit}");
            };

            let author_mail = renames
                .get(&info.author_mail)
                .unwrap_or(&info.author_mail)
                .to_string();

            *count.entry(author_mail).or_default() += amount;
        }
    }

    let mut count = count.into_iter().map(|(a, n)| (n, a)).collect::<Vec<_>>();
    count.sort_unstable();

    for (n, a) in count {
        let n = format!("{n}");
        let space = (78 - a.len() - n.len()).max(1);
        println!("{a} {} {n}", ".".repeat(space));
    }

    Ok(())
}
