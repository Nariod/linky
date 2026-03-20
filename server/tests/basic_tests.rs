// Basic integration tests for Linky C2 Framework
// Tests the core functionality without requiring network

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    
    // Import local modules directly
    mod c2;
    mod utils;
    
    use c2::{C2Config, C2Server, Implant, ImplantStatus};
    use utils::{base64_encode, generate_implant_id};

    #[test]
    fn test_c2_server_initialization() {
        let config = C2Config::default();
        let server = Arc::new(C2Server::new(config.clone()));
        
        assert_eq!(server.config.server_address, "0.0.0.0");
        assert_eq!(server.config.port, 443);
        assert!(server.get_implants().is_empty());
    }

    #[test]
    fn test_implant_creation() {
        let implant = Implant {
            id: generate_implant_id(),
            hostname: "TEST-HOST".to_string(),
            username: "testuser".to_string(),
            ip_address: "192.168.1.100".to_string(),
            platform: "TestOS".to_string(),
            last_checkin: chrono::Utc::now(),
            status: ImplantStatus::Active,
            tasks: Vec::new(),
        };

        assert!(!implant.id.to_string().is_empty());
        assert_eq!(implant.hostname, "TEST-HOST");
        assert_eq!(implant.status, ImplantStatus::Active);
    }

    #[test]
    fn test_implant_addition() {
        let config = C2Config::default();
        let server = Arc::new(C2Server::new(config));
        
        let implant = Implant {
            id: generate_implant_id(),
            hostname: "TEST".to_string(),
            username: "user".to_string(),
            ip_address: "127.0.0.1".to_string(),
            platform: "Test".to_string(),
            last_checkin: chrono::Utc::now(),
            status: ImplantStatus::Active,
            tasks: Vec::new(),
        };

        server.add_implant(implant.clone());
        let implants = server.get_implants();
        
        assert_eq!(implants.len(), 1);
        assert_eq!(implants[0].hostname, "TEST");
    }

    #[test]
    fn test_base64_encoding() {
        let test_data = "Hello from Linky";
        let encoded = base64_encode(test_data.as_bytes());
        
        assert!(!encoded.is_empty());
        assert_ne!(encoded, test_data);
    }

    #[test]
    fn test_implant_id_generation() {
        let id1 = generate_implant_id();
        let id2 = generate_implant_id();
        
        assert_ne!(id1, id2);
        assert!(!id1.to_string().is_empty());
    }

    #[test]
    fn test_config_defaults() {
        let config = C2Config::default();
        
        assert_eq!(config.server_address, "0.0.0.0");
        assert_eq!(config.port, 443);
        assert!(config.max_connections > 0);
    }
}
