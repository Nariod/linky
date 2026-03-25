// Test d'intégration pour vérifier le flux complet des commandes
// Ce test utilise le code réel de l'application

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;
    use crate::links::{Links, LinkStatus};
    use crate::tasks::TaskStatus;
    use crate::cli::show_completed_task_results;

    #[test]
    fn test_complete_command_flow() {
        println!("\n=== Test d'intégration: Flux complet des commandes ===");
        
        // Setup
        let links = Arc::new(Mutex::new(Links::new()));
        let link_id = Uuid::new_v4();
        
        // Étape 1: Créer un link (simule l'arrivée d'un implant)
        {
            let mut links_guard = links.lock().unwrap();
            links_guard.add_link(
                link_id,
                "test-link".to_string(),
                "test-user".to_string(),
                "test-host".to_string(),
                "127.0.0.1".to_string(),
                "linux".to_string(),
                1234,
            );
            println!("✅ Étape 1: Link créé");
        }
        
        // Étape 2: Ajouter une tâche (simule l'exécution d'une commande par l'utilisateur)
        let task_id;
        {
            let mut links_guard = links.lock().unwrap();
            task_id = links_guard.add_task(link_id, "whoami".to_string(), "whoami".to_string()).unwrap();
            println!("✅ Étape 2: Tâche 'whoami' ajoutée (ID: {})", task_id);
        }
        
        // Étape 3: Vérifier l'état initial
        {
            let links_guard = links.lock().unwrap();
            let link = links_guard.get_link(link_id).unwrap();
            let task = link.tasks.iter().find(|t| t.id == task_id).unwrap();
            println!("📋 État initial: status={:?}, output='{}'", task.status, task.output);
            assert_eq!(task.status, TaskStatus::Waiting);
            assert!(task.output.is_empty());
        }
        
        // Étape 4: Compléter la tâche (simule la réponse de l'implant)
        {
            let mut links_guard = links.lock().unwrap();
            links_guard.complete_task(link_id, task_id, "test-user@test-host".to_string());
            println!("✅ Étape 4: Tâche complétée avec output: 'test-user@test-host'");
        }
        
        // Étape 5: Vérifier l'état final
        {
            let links_guard = links.lock().unwrap();
            let link = links_guard.get_link(link_id).unwrap();
            let task = link.tasks.iter().find(|t| t.id == task_id).unwrap();
            println!("📋 État final: status={:?}, output='{}'", task.status, task.output);
            assert_eq!(task.status, TaskStatus::Completed);
            assert_eq!(task.output, "test-user@test-host");
        }
        
        // Étape 6: Tester l'affichage des résultats (simule l'entrée dans le menu d'interaction)
        println!("📋 Test de l'affichage des résultats:");
        show_completed_task_results(&links, link_id);
        
        // Étape 7: Vérifier que la tâche serait affichée
        {
            let links_guard = links.lock().unwrap();
            let link = links_guard.get_link(link_id).unwrap();
            let task = link.tasks.iter().find(|t| t.id == task_id).unwrap();
            
            if task.status == TaskStatus::Completed && !task.output.is_empty() {
                println!("✅ Étape 7: La tâche serait affichée à l'utilisateur");
            } else {
                println!("❌ Étape 7: La tâche ne serait PAS affichée");
                panic!("La tâche devrait être affichée mais ne l'est pas");
            }
        }
        
        println!("✅ Test terminé avec succès!\n");
    }
    
    #[test]
    fn test_multiple_commands() {
        println!("\n=== Test: Plusieurs commandes ===");
        
        let links = Arc::new(Mutex::new(Links::new()));
        let link_id = Uuid::new_v4();
        
        // Créer un link
        {
            let mut links_guard = links.lock().unwrap();
            links_guard.add_link(
                link_id,
                "test-link".to_string(),
                "test-user".to_string(),
                "test-host".to_string(),
                "127.0.0.1".to_string(),
                "linux".to_string(),
                1234,
            );
        }
        
        // Ajouter plusieurs commandes
        let command_data = vec![
            ("whoami", "test-user@test-host"),
            ("pwd", "/home/test-user"),
            ("pid", "12345"),
        ];
        
        let mut task_ids = Vec::new();
        
        // Ajouter les tâches
        for (cmd, _) in &command_data {
            let task_id;
            {
                let mut links_guard = links.lock().unwrap();
                task_id = links_guard.add_task(link_id, cmd.to_string(), cmd.to_string()).unwrap();
            }
            task_ids.push(task_id);
        }
        
        println!("✅ {} commandes ajoutées", command_data.len());
        
        // Compléter les tâches
        for (i, (_, output)) in command_data.iter().enumerate() {
            {
                let mut links_guard = links.lock().unwrap();
                links_guard.complete_task(link_id, task_ids[i], output.to_string());
            }
        }
        
        println!("✅ Toutes les commandes complétées");
        
        // Tester l'affichage
        println!("📋 Affichage des résultats:");
        show_completed_task_results(&links, link_id);
        
        // Vérifier que toutes les tâches sont complétées
        {
            let links_guard = links.lock().unwrap();
            let link = links_guard.get_link(link_id).unwrap();
            
            let completed_count = link.tasks.iter()
                .filter(|t| t.status == TaskStatus::Completed && !t.output.is_empty())
                .count();
            
            assert_eq!(completed_count, command_data.len());
            println!("✅ Toutes les {} commandes sont complétées et seraient affichées", completed_count);
        }
        
        println!("✅ Test terminé avec succès!\n");
    }
}
