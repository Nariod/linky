use colored::Colorize;
use std::io::IsTerminal as _;

/// Check if we're running in a terminal that supports colors
fn should_use_colors() -> bool {
    // Check if NO_COLOR is set or if we're not in a terminal
    std::env::var("NO_COLOR").is_err() && std::io::stdout().is_terminal()
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

/// Print a message in cyan color with bold styling
pub fn print_cyan_bold(message: &str) {
    if should_use_colors() {
        println!("{}", message.cyan().bold());
    } else {
        println!("{}", message.clear());
    }
}
