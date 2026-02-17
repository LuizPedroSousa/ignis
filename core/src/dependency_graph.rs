use std::collections::{HashMap, HashSet, VecDeque};
use crate::stage::Stage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    CircularDependency(Vec<Stage>),
    StageNotFound(Stage),
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::CircularDependency(cycle) => {
                write!(f, "Circular dependency detected: ")?;
                for (i, stage) in cycle.iter().enumerate() {
                    if i > 0 {
                        write!(f, " -> ")?;
                    }
                    write!(f, "{}", stage)?;
                }
                Ok(())
            }
            GraphError::StageNotFound(stage) => {
                write!(f, "Stage not found in graph: {}", stage)
            }
        }
    }
}

impl std::error::Error for GraphError {}

pub struct StageDependencyGraph {
    adjacency: HashMap<Stage, Vec<Stage>>,
    in_degree: HashMap<Stage, usize>,
    stages: HashSet<Stage>,
}

impl StageDependencyGraph {
    pub fn new() -> Self {
        Self {
            adjacency: HashMap::new(),
            in_degree: HashMap::new(),
            stages: HashSet::new(),
        }
    }

    pub fn from_stages(stages: Vec<Stage>) -> Self {
        let mut graph = Self::new();

        for stage in &stages {
            graph.add_stage(*stage);
        }

        for stage in &stages {
            for dep in stage.default_dependencies() {
                if stages.contains(&dep) {
                    graph.add_dependency(dep, *stage);
                }
            }
        }

        graph
    }

    pub fn add_stage(&mut self, stage: Stage) {
        self.stages.insert(stage);
        self.adjacency.entry(stage).or_insert_with(Vec::new);
        self.in_degree.entry(stage).or_insert(0);
    }

    pub fn add_dependency(&mut self, from: Stage, to: Stage) {
        self.add_stage(from);
        self.add_stage(to);

        self.adjacency.entry(from).or_default().push(to);

        *self.in_degree.entry(to).or_insert(0) += 1;
    }

    pub fn topological_sort(&self) -> Result<Vec<Vec<Stage>>, GraphError> {
        let mut in_degree = self.in_degree.clone();
        let mut queue: VecDeque<Stage> = VecDeque::new();
        let mut layers: Vec<Vec<Stage>> = Vec::new();
        let mut processed = 0;

        for (stage, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(*stage);
            }
        }

        while !queue.is_empty() {
            let current_layer_size = queue.len();
            let mut current_layer = Vec::new();

            for _ in 0..current_layer_size {
                if let Some(stage) = queue.pop_front() {
                    current_layer.push(stage);
                    processed += 1;

                    if let Some(dependents) = self.adjacency.get(&stage) {
                        for &dependent in dependents {
                            if let Some(degree) = in_degree.get_mut(&dependent) {
                                *degree -= 1;
                                if *degree == 0 {
                                    queue.push_back(dependent);
                                }
                            }
                        }
                    }
                }
            }

            if !current_layer.is_empty() {
                layers.push(current_layer);
            }
        }

        if processed != self.stages.len() {
            let remaining: Vec<Stage> = self.stages
                .iter()
                .filter(|s| in_degree.get(s).map_or(false, |&d| d > 0))
                .copied()
                .collect();
            return Err(GraphError::CircularDependency(remaining));
        }

        Ok(layers)
    }

    pub fn stages(&self) -> Vec<Stage> {
        self.stages.iter().copied().collect()
    }

    pub fn get_dependencies(&self, stage: Stage) -> Vec<Stage> {
        let mut deps = Vec::new();

        for (from_stage, to_stages) in &self.adjacency {
            if to_stages.contains(&stage) {
                deps.push(*from_stage);
            }
        }

        deps
    }
}

impl Default for StageDependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_linear_dependency() {
        let stages = vec![Stage::Configure, Stage::Build, Stage::Install];
        let graph = StageDependencyGraph::from_stages(stages);

        let result = graph.topological_sort();
        assert!(result.is_ok());

        let layers = result.unwrap();
        assert_eq!(layers.len(), 3);
        assert_eq!(layers[0], vec![Stage::Configure]);
        assert_eq!(layers[1], vec![Stage::Build]);
        assert_eq!(layers[2], vec![Stage::Install]);
    }

    #[test]
    fn test_concurrent_stages() {
        let stages = vec![Stage::Build, Stage::Install, Stage::PostBuild];
        let graph = StageDependencyGraph::from_stages(stages);

        let result = graph.topological_sort();
        assert!(result.is_ok());

        let layers = result.unwrap();
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0], vec![Stage::Build]);
        assert_eq!(layers[1].len(), 2);
        assert!(layers[1].contains(&Stage::Install));
        assert!(layers[1].contains(&Stage::PostBuild));
    }

    #[test]
    fn test_circular_dependency() {
        let mut graph = StageDependencyGraph::new();
        graph.add_stage(Stage::Build);
        graph.add_stage(Stage::Configure);

        graph.add_dependency(Stage::Build, Stage::Configure);
        graph.add_dependency(Stage::Configure, Stage::Build);

        let result = graph.topological_sort();
        assert!(result.is_err());

        if let Err(GraphError::CircularDependency(_)) = result {
        } else {
            panic!("Expected CircularDependency error");
        }
    }

    #[test]
    fn test_empty_graph() {
        let graph = StageDependencyGraph::new();
        let result = graph.topological_sort();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![] as Vec<Vec<Stage>>);
    }

    #[test]
    fn test_single_stage() {
        let mut graph = StageDependencyGraph::new();
        graph.add_stage(Stage::Build);

        let result = graph.topological_sort();
        assert!(result.is_ok());

        let layers = result.unwrap();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0], vec![Stage::Build]);
    }

    #[test]
    fn test_complex_dependency_graph() {
        let stages = vec![
            Stage::PreValidation,
            Stage::Configure,
            Stage::Build,
            Stage::Install,
            Stage::PostBuild,
        ];
        let graph = StageDependencyGraph::from_stages(stages);

        let result = graph.topological_sort();
        assert!(result.is_ok());

        let layers = result.unwrap();
        assert_eq!(layers.len(), 4);
        assert_eq!(layers[0], vec![Stage::PreValidation]);
        assert_eq!(layers[1], vec![Stage::Configure]);
        assert_eq!(layers[2], vec![Stage::Build]);
        assert_eq!(layers[3].len(), 2);
        assert!(layers[3].contains(&Stage::Install));
        assert!(layers[3].contains(&Stage::PostBuild));
    }

    #[test]
    fn test_get_dependencies() {
        let stages = vec![Stage::Configure, Stage::Build];
        let graph = StageDependencyGraph::from_stages(stages);

        let deps = graph.get_dependencies(Stage::Build);
        assert_eq!(deps, vec![Stage::Configure]);

        let deps = graph.get_dependencies(Stage::Configure);
        assert_eq!(deps.len(), 0);
    }
}
