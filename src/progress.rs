use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

const TEMPLATE: &str = "{bar:40} {percent}% | {prefix} | {decimal_bytes}/{decimal_total_bytes} | {decimal_bytes_per_sec} | ETA {eta}";
const VERIFY_TEMPLATE: &str = "{bar:40} {percent}% | verificando | {pos}/{len}";

fn bar_style() -> ProgressStyle {
    ProgressStyle::with_template(TEMPLATE)
        .expect("invalid progress template")
        .progress_chars("■■ ")
}

pub struct Progress {
    multi: MultiProgress,
    overall: ProgressBar,
}

impl Progress {
    pub fn new(total_bytes: u64) -> Self {
        let multi = MultiProgress::new();
        let overall = multi.add(ProgressBar::new(total_bytes));
        overall.set_style(bar_style());
        overall.set_prefix("total");
        Self { multi, overall }
    }

    pub fn add_mod(&self, name: &str, size: u64) -> ProgressBar {
        let bar = self
            .multi
            .insert_before(&self.overall, ProgressBar::new(size));
        bar.set_style(bar_style());
        bar.set_prefix(name.to_string());
        bar
    }

    pub fn overall(&self) -> &ProgressBar {
        &self.overall
    }

    pub fn finish(&self, success: bool) {
        if success {
            self.overall.finish();
        } else {
            self.overall.abandon();
        }
    }
}

pub fn verification_bar(total: u64) -> ProgressBar {
    let bar = ProgressBar::new(total);
    bar.set_style(
        ProgressStyle::with_template(VERIFY_TEMPLATE)
            .expect("invalid progress template")
            .progress_chars("■■ "),
    );
    bar
}
