//! OS dialog boundary shared by optional tools; workflow tests supply selections.
use std::path::PathBuf;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Open,
    Save,
    Folder,
}
pub fn choose(kind: Kind) -> Option<PathBuf> {
    #[cfg(test)]
    if let Some(selection) = SELECTIONS.with(|selections| {
        selections.borrow_mut().as_mut().map(|queue| {
            let (expected, value) = queue.pop_front().expect("Unexpected tool dialog");
            assert_eq!(expected, kind);
            value
        })
    }) {
        return selection;
    }
    match kind {
        Kind::Open => rfd::FileDialog::new().pick_file(),
        Kind::Save => rfd::FileDialog::new().save_file(),
        Kind::Folder => rfd::FileDialog::new().pick_folder(),
    }
}
pub fn open() -> Option<PathBuf> {
    choose(Kind::Open)
}
pub fn save() -> Option<PathBuf> {
    choose(Kind::Save)
}
pub fn folder() -> Option<PathBuf> {
    choose(Kind::Folder)
}
#[cfg(test)]
type Queue = std::collections::VecDeque<(Kind, Option<PathBuf>)>;
#[cfg(test)]
thread_local! {static SELECTIONS:std::cell::RefCell<Option<Queue>>=const{std::cell::RefCell::new(None)};}
#[cfg(test)]
pub struct Selections;
#[cfg(test)]
impl Selections {
    pub fn new(values: Vec<(Kind, Option<PathBuf>)>) -> Self {
        SELECTIONS.with(|state| {
            assert!(state.borrow().is_none());
            *state.borrow_mut() = Some(values.into());
        });
        Self
    }
}
#[cfg(test)]
impl Drop for Selections {
    fn drop(&mut self) {
        SELECTIONS.with(|state| {
            let remaining = state.borrow_mut().take();
            if !std::thread::panicking() {
                assert!(
                    remaining.unwrap().is_empty(),
                    "Unused tool dialog selections"
                );
            }
        });
    }
}
