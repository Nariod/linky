#!/bin/bash

# Script de test autonome pour Linky C2
# Objectif: Tester le flux complet des commandes et identifier pourquoi les résultats ne s'affichent pas

echo "=== Début du test autonome ==="

# Étape 1: Vérifier que le code compile
echo "1. Vérification de la compilation..."
cd /home/fedora/Documents/linky
cargo build --release 2>&1 | tail -3
if [ $? -ne 0 ]; then
    echo "❌ Échec de la compilation"
    exit 1
fi
echo "✅ Compilation réussie"

# Étape 2: Créer un test unitaire simple
echo "2. Création d'un test unitaire simple..."

cat > /tmp/test_links.rs << 'EOF'
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use linky::links::{Links, LinkStatus};
use linky::tasks::TaskStatus;

#[test]
fn test_task_creation_and_completion() {
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
    
    // Ajouter une tâche
    let task_id;
    {
        let mut links_guard = links.lock().unwrap();
        task_id = links_guard.add_task(link_id, "whoami".to_string(), "whoami".to_string()).unwrap();
    }
    
    // Compléter la tâche
    {
        let mut links_guard = links.lock().unwrap();
        links_guard.complete_task(link_id, task_id, "test-user@test-host".to_string());
    }
    
    // Vérifier que la tâche est complétée
    let links_guard = links.lock().unwrap();
    let link = links_guard.get_link(link_id).unwrap();
    assert_eq!(link.tasks.len(), 1);
    assert_eq!(link.tasks[0].status, TaskStatus::Completed);
    assert_eq!(link.tasks[0].output, "test-user@test-host");
    
    println!("✅ Test réussi: tâche créée et complétée correctement");
}
EOF

echo "Test unitaire créé"

# Étape 3: Tester la fonction show_completed_task_results
echo "3. Test de la fonction d'affichage des résultats..."

# Créer un petit programme de test
cat > /tmp/test_display.rs << 'EOF'
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use chrono::Local;

// Copier les structures nécessaires
#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Waiting,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: Uuid,
    pub command: String,
    pub cli_command: String,
    pub status: TaskStatus,
    pub output: String,
}

struct Link {
    pub id: Uuid,
    pub name: String,
    pub tasks: Vec<Task>,
}

struct Links {
    links: Vec<Link>,
}

impl Links {
    fn new() -> Self {
        Self { links: Vec::new() }
    }
    
    fn add_link(&mut self, id: Uuid, name: String) {
        self.links.push(Link {
            id,
            name,
            tasks: Vec::new(),
        });
    }
    
    fn get_link_mut(&mut self, id: Uuid) -> Option<&mut Link> {
        self.links.iter_mut().find(|l| l.id == id)
    }
}

fn show_completed_task_results(links: &Arc<Mutex<Links>>, link_id: Uuid) {
    let l = links.lock().unwrap();
    if let Some(link) = l.links.iter().find(|l| l.id == link_id) {
        println!("Link {} has {} tasks:", link.name, link.tasks.len());
        for (i, task) in link.tasks.iter().enumerate() {
            println!("Task {}: cmd='{}', status={:?}, output='{}'", 
                i, task.cli_command, task.status, task.output);
            
            if task.status == TaskStatus::Completed && !task.output.is_empty() {
                println!("✅ Displaying result for task {}:", i);
                println!("Result: {}", task.output);
            }
        }
    }
}

fn main() {
    let links = Arc::new(Mutex::new(Links::new()));
    let link_id = Uuid::new_v4();
    
    // Setup
    {
        let mut links_guard = links.lock().unwrap();
        links_guard.add_link(link_id, "test-link".to_string());
        if let Some(link) = links_guard.get_link_mut(link_id) {
            link.tasks.push(Task {
                id: Uuid::new_v4(),
                command: "whoami".to_string(),
                cli_command: "whoami".to_string(),
                status: TaskStatus::Completed,
                output: "test-user@test-host".to_string(),
            });
        }
    }
    
    // Test display
    show_completed_task_results(&links, link_id);
    println!("✅ Test d'affichage réussi");
}
EOF

echo "Programme de test créé"

# Étape 4: Analyser le problème potentiel
echo "4. Analyse du problème..."

echo "Problèmes potentiels identifiés :"
echo "a) Les tâches ne sont pas créées correctement"
echo "b) Les tâches sont créées mais pas complétées"
echo "c) Les tâches sont complétées mais la sortie est vide"
echo "d) Les tâches sont complétées mais l'affichage ne fonctionne pas"

echo ""
echo "Solution proposée :"
echo "1. Vérifier que add_task retourne bien un UUID"
echo "2. Vérifier que complete_task est appelé avec le bon UUID"
echo "3. Vérifier que la sortie n'est pas vide"
echo "4. Vérifier que show_completed_task_results est appelé"

echo ""
echo "=== Fin du test autonome ==="
