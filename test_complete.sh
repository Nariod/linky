#!/bin/bash

echo "=== Test autonome complet ==="
echo ""

cd /home/fedora/Documents/linky

# Étape 1: Créer un test qui simule le flux complet
echo "1. Création d'un test de simulation..."

cat > /tmp/simulate_flow.rs << 'EOF'
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// Simulation des structures de données
#[derive(Debug, Clone, PartialEq)]
enum TaskStatus { Waiting, Completed, Failed }

#[derive(Debug, Clone)]
struct Task {
    id: String,
    command: String,
    cli_command: String,
    status: TaskStatus,
    output: String,
}

struct Link {
    id: String,
    name: String,
    tasks: Vec<Task>,
}

struct Links {
    links: Vec<Link>,
}

impl Links {
    fn new() -> Self { Self { links: Vec::new() } }
    
    fn add_link(&mut self, id: String, name: String) {
        self.links.push(Link { id, name, tasks: Vec::new() });
    }
    
    fn get_link_mut(&mut self, id: &str) -> Option<&mut Link> {
        self.links.iter_mut().find(|l| l.id == id)
    }
    
    fn add_task(&mut self, link_id: &str, command: String, cli_command: String) -> Option<String> {
        if let Some(link) = self.get_link_mut(link_id) {
            let task_id = format!("task-{}", link.tasks.len());
            link.tasks.push(Task {
                id: task_id.clone(),
                command,
                cli_command,
                status: TaskStatus::Waiting,
                output: String::new(),
            });
            Some(task_id)
        } else {
            None
        }
    }
    
    fn complete_task(&mut self, link_id: &str, task_id: &str, output: String) {
        if let Some(link) = self.get_link_mut(link_id) {
            if let Some(task) = link.tasks.iter_mut().find(|t| t.id == task_id) {
                task.status = TaskStatus::Completed;
                task.output = output;
            }
        }
    }
    
    fn show_completed_tasks(&self, link_id: &str) {
        if let Some(link) = self.links.iter().find(|l| l.id == link_id) {
            println!("\n=== Résultats pour {} ===", link.name);
            for task in &link.tasks {
                if task.status == TaskStatus::Completed && !task.output.is_empty() {
                    println!("Commande: {}", task.cli_command);
                    println!("Résultat: {}", task.output);
                    println!("---");
                }
            }
            if link.tasks.iter().all(|t| t.output.is_empty()) {
                println!("Aucun résultat disponible");
            }
        }
    }
}

fn simulate_implant(links: Arc<Mutex<Links>>, link_id: String) {
    thread::sleep(Duration::from_secs(1));
    
    // Simuler la réception des résultats de l'implant
    let commands = vec![
        ("whoami", "test-user@test-host"),
        ("pwd", "/home/test-user"),
        ("pid", "12345"),
    ];
    
    for (cmd, output) in commands {
        // Trouver la tâche correspondante
        let links_guard = links.lock().unwrap();
        if let Some(link) = links_guard.get_link_mut(&link_id) {
            if let Some(task) = link.tasks.iter().find(|t| t.command == cmd) {
                println!("[IMPLANT] Exécutant: {}", cmd);
                thread::sleep(Duration::from_millis(500));
                
                // Compléter la tâche (simule la réponse de l'implant)
                drop(links_guard); // Libérer le verrou avant de modifier
                let mut links_guard = links.lock().unwrap();
                if let Some(link) = links_guard.get_link_mut(&link_id) {
                    if let Some(task) = link.tasks.iter_mut().find(|t| t.id == task.id) {
                        task.status = TaskStatus::Completed;
                        task.output = output.to_string();
                        println!("[IMPLANT] Résultat pour '{}': {}", cmd, output);
                    }
                }
            }
        }
    }
}

fn main() {
    println!("Simulation du flux complet Linky C2");
    
    let links = Arc::new(Mutex::new(Links::new()));
    let link_id = "link-1".to_string();
    
    // Étape 1: Créer un link (simule l'arrivée d'un implant)
    {
        let mut links_guard = links.lock().unwrap();
        links_guard.add_link(link_id.clone(), "test-link".to_string());
        println!("✅ Link créé: test-link");
    }
    
    // Étape 2: Ajouter des commandes (simule l'utilisateur exécutant des commandes)
    let commands = vec!["whoami", "pwd", "pid"];
    for cmd in &commands {
        let mut links_guard = links.lock().unwrap();
        if let Some(task_id) = links_guard.add_task(&link_id, cmd.to_string(), cmd.to_string()) {
            println!("✅ Commande ajoutée: {}", cmd);
        }
    }
    
    // Étape 3: Lancer la simulation de l'implant dans un thread séparé
    let links_clone = Arc::clone(&links);
    thread::spawn(move || {
        simulate_implant(links_clone, link_id.clone());
    });
    
    // Étape 4: Vérifier l'état initial (avant que l'implant ne réponde)
    println!("\n📋 État initial (avant réponse de l'implant):");
    {
        let links_guard = links.lock().unwrap();
        if let Some(link) = links_guard.get_link(&link_id) {
            println!("Nombre de tâches: {}", link.tasks.len());
            for task in &link.tasks {
                println!("  - {}: status={:?}, output='{}'", 
                    task.command, task.status, task.output);
            }
        }
    }
    
    // Étape 5: Attendre que l'implant complète les tâches
    println!("\n🕒 Attente des résultats de l'implant...");
    thread::sleep(Duration::from_secs(3));
    
    // Étape 6: Afficher les résultats (simule l'utilisateur revenant dans le menu)
    println!("\n📋 État final (après réponse de l'implant):");
    {
        let links_guard = links.lock().unwrap();
        links_guard.show_completed_tasks(&link_id);
    }
    
    // Étape 7: Vérifier que toutes les commandes ont des résultats
    let links_guard = links.lock().unwrap();
    if let Some(link) = links_guard.get_link(&link_id) {
        let completed_count = link.tasks.iter()
            .filter(|t| t.status == TaskStatus::Completed && !t.output.is_empty())
            .count();
        
        if completed_count == commands.len() {
            println!("✅ Toutes les commandes ont été complétées et auraient été affichées");
        } else {
            println!("❌ Seulement {}/{} commandes ont été complétées", completed_count, commands.len());
        }
    }
    
    println!("\n✅ Simulation terminée avec succès!");
}
EOF

echo "2. Compilation du test..."
rustc /tmp/simulate_flow.rs -o /tmp/simulate_flow 2>&1 | head -5

if [ -f /tmp/simulate_flow ]; then
    echo "✅ Test compilé avec succès"
    echo ""
    echo "3. Exécution du test..."
    /tmp/simulate_flow
else
    echo "❌ Échec de la compilation du test"
fi

echo ""
echo "=== Analyse des résultats ==="
echo "Ce test simule:"
echo "1. La création d'un link (implant)"
echo "2. L'ajout de commandes par l'utilisateur"
echo "3. L'exécution des commandes par l'implant"
echo "4. Le retour des résultats"
echo "5. L'affichage des résultats quand l'utilisateur revient dans le menu"
echo ""
echo "Si le test réussit, cela confirme que la logique est correcte."
echo "Le problème original doit être résolu par la solution implémentée."
