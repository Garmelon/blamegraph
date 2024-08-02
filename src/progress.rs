use indicatif::{ProgressBar, ProgressStyle};

pub fn counting_bar(name: impl ToString, initial: usize) -> ProgressBar {
    ProgressBar::new(initial.try_into().unwrap())
        .with_message(name.to_string())
        .with_style(ProgressStyle::with_template("{msg}: {pos}/{len}").unwrap())
}

pub fn commit_blame_bar(hash: impl ToString, initial: usize) -> ProgressBar {
    ProgressBar::new(initial.try_into().unwrap())
        .with_message(hash.to_string())
        .with_style(ProgressStyle::with_template("{msg:40} {bar:36} {percent:>3}%").unwrap())
}
