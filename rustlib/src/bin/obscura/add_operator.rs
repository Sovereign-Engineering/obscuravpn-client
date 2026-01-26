use std::env::var;
use std::process::exit;

pub async fn run_add_operator(users: Vec<String>) -> ! {
    tokio::task::spawn_blocking(move || add_operator_impl(users)).await.unwrap()
}

fn add_operator_impl(mut users: Vec<String>) -> ! {
    if users.is_empty() {
        let Ok(user) =
            var("USER").inspect_err(|error| tracing::error!(message_id = "vo2NOhH3", ?error, "failed to read $USER environment variable: {error}"))
        else {
            eprintln!("Could not determine the current user. Please specify a user explicitly:");
            eprintln!("obscura add-operator <user>");
            exit(1);
        };
        users.push(user);
    }

    let mut failed_any = false;
    for user in &users {
        let command = ["sudo", "usermod", "-a", "-G", "obscura", user.as_str()];
        let failed = std::process::Command::new(command[0])
            .args(&command[1..])
            .status()
            .map_err(|error| tracing::error!(message_id = "uHdEDIlq", ?error, "failed to run {}: {error}", command[0]))
            .and_then(|status| status.success().then_some(()).ok_or(()))
            .is_err();
        failed_any |= failed;
        if failed {
            match shlex::try_join(command) {
                Ok(quoted_command) => eprintln!("Failed to add '{user}' to 'obscura' group using:\n    {quoted_command}"),
                Err(_) => eprintln!("Failed to add {user}"),
            }
        } else {
            eprintln!("Added {user} to 'obscura' group.")
        }
    }

    if failed_any { exit(1) } else { exit(0) }
}
