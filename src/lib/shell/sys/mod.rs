pub mod signals;

#[cfg(target_os = "redox")]
pub const NULL_PATH: &str = "null:";
#[cfg(unix)]
pub const NULL_PATH: &str = "/dev/null";

pub mod variables {
    use users::{get_user_by_name, os::unix::UserExt};

    pub fn get_user_home(username: &str) -> Option<String> {
        match get_user_by_name(username) {
            Some(user) => Some(user.home_dir().to_string_lossy().into_owned()),
            None => None,
        }
    }
}
