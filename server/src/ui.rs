use colored::Colorize;

/// Check if we're running in a terminal that supports colors
fn should_use_colors() -> bool {
    // Check if NO_COLOR is set or if we're not in a terminal
    std::env::var("NO_COLOR").is_err() && atty::is(atty::Stream::Stdout)
}

/// Print a message to the console without using tracing (for UI output)
pub fn print(message: &str) {
    if should_use_colors() {
        println!("{}", message);
    } else {
        println!("{}", message.clear());
    }
}

/// Print a formatted message with bold styling
pub fn print_bold(message: &str) {
    if should_use_colors() {
        println!("{}", message.bold());
    } else {
        println!("{}", message.clear());
    }
}

/// Print a message in cyan color
pub fn print_cyan(message: &str) {
    if should_use_colors() {
        println!("{}", message.cyan());
    } else {
        println!("{}", message.clear());
    }
}

/// Print a message in cyan color with bold styling
pub fn print_cyan_bold(message: &str) {
    if should_use_colors() {
        println!("{}", message.cyan().bold());
    } else {
        println!("{}", message.clear());
    }
}

/// Print a message in green color
pub fn print_green(message: &str) {
    if should_use_colors() {
        println!("{}", message.green());
    } else {
        println!("{}", message.clear());
    }
}

/// Print a message in red color
pub fn print_red(message: &str) {
    if should_use_colors() {
        println!("{}", message.red());
    } else {
        println!("{}", message.clear());
    }
}

/// Print a message in yellow color
pub fn print_yellow(message: &str) {
    if should_use_colors() {
        println!("{}", message.yellow());
    } else {
        println!("{}", message.clear());
    }
}