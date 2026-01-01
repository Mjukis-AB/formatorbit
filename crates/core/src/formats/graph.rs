//! Graph format (DOT and Mermaid).
//!
//! Parses Graphviz DOT and Mermaid diagram definitions.

use crate::format::{Format, FormatInfo};
use crate::types::{CoreValue, Interpretation, RichDisplay, RichDisplayOption};

pub struct GraphFormat;

impl GraphFormat {
    /// Check if input looks like a DOT graph.
    fn parse_dot(input: &str) -> Option<Interpretation> {
        let trimmed = input.trim();

        // DOT graphs start with graph/digraph/strict
        let lower = trimmed.to_lowercase();
        let is_dot = lower.starts_with("digraph ")
            || lower.starts_with("graph ")
            || lower.starts_with("strict digraph ")
            || lower.starts_with("strict graph ");

        if !is_dot {
            return None;
        }

        // Basic validation: must have braces
        if !trimmed.contains('{') || !trimmed.contains('}') {
            return None;
        }

        // Extract graph name if present
        let description = if let Some(name) = Self::extract_dot_name(trimmed) {
            format!("DOT graph: {name}")
        } else {
            "DOT graph".to_string()
        };

        Some(Interpretation {
            value: CoreValue::String(input.to_string()),
            source_format: "dot".to_string(),
            confidence: 0.90,
            description,
            rich_display: vec![RichDisplayOption::new(RichDisplay::Dot {
                source: input.to_string(),
            })],
        })
    }

    /// Extract graph name from DOT definition.
    fn extract_dot_name(input: &str) -> Option<String> {
        // Match: digraph NAME { or graph NAME {
        let input = input.trim();
        let after_keyword = if input.to_lowercase().starts_with("strict ") {
            input[7..].trim_start()
        } else {
            input
        };

        let after_type = if after_keyword.to_lowercase().starts_with("digraph ") {
            &after_keyword[8..]
        } else if after_keyword.to_lowercase().starts_with("graph ") {
            &after_keyword[6..]
        } else {
            return None;
        };

        let name = after_type.trim_start().split('{').next()?.trim();
        if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    }

    /// Check if input looks like a Mermaid diagram.
    fn parse_mermaid(input: &str) -> Option<Interpretation> {
        let trimmed = input.trim();
        let first_line = trimmed.lines().next()?.trim().to_lowercase();

        // Mermaid diagrams start with specific keywords
        let diagram_type =
            if first_line.starts_with("graph ") || first_line.starts_with("flowchart ") {
                "flowchart"
            } else if first_line.starts_with("sequencediagram") || first_line == "sequencediagram" {
                "sequence diagram"
            } else if first_line.starts_with("classDiagram") || first_line == "classdiagram" {
                "class diagram"
            } else if first_line.starts_with("statediagram") || first_line == "statediagram" {
                "state diagram"
            } else if first_line.starts_with("erdiagram") || first_line == "erdiagram" {
                "ER diagram"
            } else if first_line.starts_with("gantt") {
                "Gantt chart"
            } else if first_line.starts_with("pie") {
                "pie chart"
            } else if first_line.starts_with("gitgraph") {
                "Git graph"
            } else if first_line.starts_with("mindmap") {
                "mindmap"
            } else if first_line.starts_with("timeline") {
                "timeline"
            } else if first_line.starts_with("journey") {
                "journey"
            } else if first_line.starts_with("quadrantchart") {
                "quadrant chart"
            } else if first_line.starts_with("requirementdiagram") {
                "requirement diagram"
            } else if first_line.starts_with("c4context")
                || first_line.starts_with("c4container")
                || first_line.starts_with("c4component")
                || first_line.starts_with("c4dynamic")
                || first_line.starts_with("c4deployment")
            {
                "C4 diagram"
            } else if first_line.starts_with("sankey") {
                "sankey diagram"
            } else if first_line.starts_with("xychart") {
                "XY chart"
            } else if first_line.starts_with("block") {
                "block diagram"
            } else {
                return None;
            };

        Some(Interpretation {
            value: CoreValue::String(input.to_string()),
            source_format: "mermaid".to_string(),
            confidence: 0.90,
            description: format!("Mermaid {diagram_type}"),
            rich_display: vec![RichDisplayOption::new(RichDisplay::Mermaid {
                source: input.to_string(),
            })],
        })
    }
}

impl Format for GraphFormat {
    fn id(&self) -> &'static str {
        "graph"
    }

    fn name(&self) -> &'static str {
        "Graph (DOT/Mermaid)"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Data",
            description: "Graphviz DOT and Mermaid diagram definitions",
            examples: &["digraph G { A -> B }", "graph TD\\n  A --> B"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Try DOT first, then Mermaid
        if let Some(interp) = Self::parse_dot(input) {
            return vec![interp];
        }

        if let Some(interp) = Self::parse_mermaid(input) {
            return vec![interp];
        }

        vec![]
    }

    fn can_format(&self, value: &CoreValue) -> bool {
        // Only format strings that are valid DOT or Mermaid graphs
        match value {
            CoreValue::String(s) => {
                Self::parse_dot(s).is_some() || Self::parse_mermaid(s).is_some()
            }
            _ => false,
        }
    }

    fn format(&self, value: &CoreValue) -> Option<String> {
        match value {
            CoreValue::String(s) => {
                // Only return if it's a valid graph
                if Self::parse_dot(s).is_some() || Self::parse_mermaid(s).is_some() {
                    Some(s.clone())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["dot", "graphviz", "mermaid"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dot_digraph() {
        let format = GraphFormat;
        let results = format.parse("digraph G { A -> B }");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "dot");
        assert!(results[0].description.contains("DOT graph"));
        assert!(results[0].description.contains("G"));
    }

    #[test]
    fn test_parse_dot_graph() {
        let format = GraphFormat;
        let results = format.parse("graph G { A -- B }");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "dot");
    }

    #[test]
    fn test_parse_dot_strict() {
        let format = GraphFormat;
        let results = format.parse("strict digraph { A -> B }");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "dot");
    }

    #[test]
    fn test_parse_dot_multiline() {
        let format = GraphFormat;
        let input = r#"digraph G {
            rankdir=LR;
            A -> B -> C;
            B -> D;
            A [label="Start"];
            C [label="End"];
        }"#;
        let results = format.parse(input);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "dot");
        assert!(results[0].description.contains("G"));

        // Check RichDisplay
        assert_eq!(results[0].rich_display.len(), 1);
        if let RichDisplay::Dot { source } = &results[0].rich_display[0].preferred {
            assert!(source.contains("rankdir=LR"));
        } else {
            panic!("Expected Dot rich display");
        }
    }

    #[test]
    fn test_parse_mermaid_flowchart() {
        let format = GraphFormat;
        let results = format.parse("graph TD\n  A --> B");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "mermaid");
        assert!(results[0].description.contains("flowchart"));
    }

    #[test]
    fn test_parse_mermaid_sequence() {
        let format = GraphFormat;
        let results = format.parse("sequenceDiagram\n  Alice->>Bob: Hello");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "mermaid");
        assert!(results[0].description.contains("sequence"));
    }

    #[test]
    fn test_parse_mermaid_pie() {
        let format = GraphFormat;
        let results = format.parse("pie title Pets\n  \"Dogs\" : 386\n  \"Cats\" : 85");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "mermaid");
        assert!(results[0].description.contains("pie"));
    }

    #[test]
    fn test_parse_mermaid_gantt() {
        let format = GraphFormat;
        let results = format.parse(
            "gantt\n  title A Gantt Diagram\n  section Section\n  A task : a1, 2024-01-01, 30d",
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "mermaid");
        assert!(results[0].description.contains("Gantt"));
    }

    #[test]
    fn test_not_graph() {
        let format = GraphFormat;
        assert!(format.parse("hello world").is_empty());
        assert!(format.parse("123").is_empty());
        assert!(format.parse("{ key: value }").is_empty());
    }

    #[test]
    fn test_rich_display_mermaid() {
        let format = GraphFormat;
        let results = format.parse("flowchart LR\n  A --> B");

        assert_eq!(results[0].rich_display.len(), 1);
        if let RichDisplay::Mermaid { source } = &results[0].rich_display[0].preferred {
            assert!(source.contains("flowchart"));
        } else {
            panic!("Expected Mermaid rich display");
        }
    }
}
