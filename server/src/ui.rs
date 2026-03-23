use colored::Colorize;

/// Print a message to the console without using tracing (for UI output)
pub fn print(message: &str) {
    println!("{}", message);
}

/// Print a formatted message with bold styling
pub fn print_bold(message: &str) {
    println!("{}", message.bold());
}

/// Print a message in cyan color
pub fn print_cyan(message: &str) {
    println!("{}", message.cyan());
}

/// Print a message in cyan color with bold styling
pub fn print_cyan_bold(message: &str) {
    println!("{}", message.cyan().bold());
}

/// Print a message in green color
pub fn print_green(message: &str) {
    println!("{}", message.green());
}

/// Print a message in red color
pub fn print_red(message: &str) {
    println!("{}", message.red());
}

/// Print a message in yellow color
pub fn print_yellow(message: &str) {
    println!("{}", message.yellow());
}