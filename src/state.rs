use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub valid_tokens: HashMap<String, String>,
    pub valid_content: Vec<Uuid>,
}

impl AppState {
    pub fn new() -> Self {
        let mut tokens = HashMap::new();
        tokens.insert(
            "550e8400-e29b-41d4-a716-446655440001".to_string(),
            "tok_user_1".to_string(),
        );
        tokens.insert(
            "550e8400-e29b-41d4-a716-446655440002".to_string(),
            "tok_user_2".to_string(),
        );
        tokens.insert(
            "550e8400-e29b-41d4-a716-446655440003".to_string(),
            "tok_user_3".to_string(),
        );

        let valid_content = vec![
            Uuid::parse_str("731b0395-4888-4822-b516-05b4b7bf2089").unwrap(),
            Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap(),
        ];

        Self {
            valid_tokens: tokens,
            valid_content,
        }
    }
}
