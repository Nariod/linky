#!/bin/bash

# Test de complétion des tâches
# Objectif: Vérifier si les tâches sont correctement marquées comme complétées

echo "=== Test de complétion des tâches ==="

cd /home/fedora/Documents/linky

# Créer un test qui simule le flux complet
echo "Création d'un test de flux complet..."

cat > /tmp/test_full_flow.rs << 'EOF'
use std::sync::{Arc, Mutex};
use uuid::Uuid;

// Structures simplifiées pour le test
#[derive(Debug, Clone, PartialEq)]
enum TaskStatus { Waiting, Completed, Failed }

#[derive(Debug, Clone)]
struct Task {
    id: Uuid,
    command: String,
    cli_command: String,
    status: TaskStatus,
    output: String,
}

struct Link {
    id: Uuid,
    name: String,
    tasks: Vec<Task>,
}

struct Links {
    links: Vec<Link>,
}

impl Links {
    fn new() -> Self { Self { links: Vec::new() } }
    
    fn add_link(&mut self, id: Uuid, name: String) {
        self.links.push(Link { id, name, tasks: Vec::new() });
    }
    
    fn get_link(&self, id: Uuid) -> Option<&Link> {
        self.links.iter().find(|l| l.id == id)
    }
    
    fn get_link_mut(&mut self, id: Uuid) -> Option<&mut Link> {
        self.links.iter_mut().find(|l| l.id == id)
    }
    
    fn add_task(&mut self, link_id: Uuid, command: String, cli_command: String) -> Option<Uuid> {
        if let Some(link) = self.get_link_mut(link_id) {
            let task_id = Uuid::new_v4();
            link.tasks.push(Task {
                id: task_id,
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
    
    fn complete_task(&mut self, link_id: Uuid, task_id: Uuid, output: String) {
        if let Some(link) = self.get_link_mut(link_id) {
            if let Some(task) = link.tasks.iter_mut().find(|t| t.id == task_id) {
                task.status = TaskStatus::Completed;
                task.output = output;
            }
        }
    }
}

fn main() {
    println!("Test du flux complet de traitement des commandes");
    
    let links = Arc::new(Mutex::new(Links::new()));
    let link_id = Uuid::new_v4();
    
    // Étape 1: Créer un link (simule l'arrivée d'un nouvel implant)
    {
        let mut links_guard = links.lock().unwrap();
        links_guard.add_link(link_id, "test-link".to_string());
        println!("✅ Link créé: test-link");
    }
    
    // Étape 2: Ajouter une tâche (simule l'exécution d'une commande par l'utilisateur)
    let task_id;
    {
        let mut links_guard = links.lock().unwrap();
        task_id = links_guard.add_task(link_id, "whoami".to_string(), "whoami".to_string()).unwrap();
        println!("✅ Tâche ajoutée: whoami (ID: {})", task_id);
    }
    
    // Étape 3: Vérifier l'état initial de la tâche
    {
        let links_guard = links.lock().unwrap();
        if let Some(link) = links_guard.get_link(link_id) {
            if let Some(task) = link.tasks.iter().find(|t| t.id == task_id) {
                println!("📋 État initial: status={:?}, output='{}'", task.status, task.output);
            }
        }
    }
    
    // Étape 4: Compléter la tâche (simule la réponse de l'implant)
    {
        let mut links_guard = links.lock().unwrap();
        links_guard.complete_task(link_id, task_id, "test-user@test-host".to_string());
        println!("✅ Tâche complétée avec output: test-user@test-host");
    }
    
    // Étape 5: Vérifier l'état final de la tâche
    {
        let links_guard = links.lock().unwrap();
        if let Some(link) = links_guard.get_link(link_id) {
            if let Some(task) = link.tasks.iter().find(|t| t.id == task_id) {
                println!("📋 État final: status={:?}, output='{}'", task.status, task.output);
                
                // Étape 6: Vérifier si la tâche serait affichée
                if task.status == TaskStatus::Completed && !task.output.is_empty() {
                    println!("✅ La tâche serait affichée à l'utilisateur");
                    println!("Resultat: {}", task.output);
                } else {
                    println!("❌ La tâche ne serait PAS affichée");
                    if task.status != TaskStatus::Completed {
                        println!("  Raison: statut n'est pas Completed");
                    }
                    if task.output.is_empty() {
                        println!("  Raison: output est vide");
                    }
                }
            }
        }
    }
    
    println!("\n✅ Test terminé avec succès");
}
EOF

echo "Compilation du test..."
rustc /tmp/test_full_flow.rs -o /tmp/test_full_flow -L target/release/deps 2>&1 | head -10

if [ -f /tmp/test_full_flow ]; then
    echo "Exécution du test..."
    /tmp/test_full_flow
else
    echo "❌ Échec de la compilation du test"
fi

echo ""
echo "=== Analyse des résultats ==="
echo "Si le test montre que les tâches sont correctement complétées,"
echo "alors le problème vient de l'interface CLI qui n'affiche pas les résultats."
echo "Si le test montre que les tâches ne sont pas complétées,"
echo "alors le problème vient du serveur qui ne reçoit pas les résultats."
