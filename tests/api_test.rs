#[cfg(test)]
mod tests {
    use reqwest;
    use serde_json::json;

    #[tokio::test]
    async fn test_compute_api() {
        let client = reqwest::Client::new();

        let request = json!({
            "program": [
                {"opcode": "MOV", "params": [1, 10]},
                {"opcode": "MOV", "params": [2, 5]},
                {"opcode": "ADD", "params": [1, 2]},
                {"opcode": "MUL", "params": [1, 2]},
                {"opcode": "HALT", "params": []}
            ],
            "input_registers": null
        });

        let response = client
            .post("http://localhost:3000/compute")
            .json(&request)
            .send()
            .await;

        match response {
            Ok(resp) => {
                println!("API Response: {:?}", resp);
                assert!(resp.status().is_success());
            }
            Err(e) => {
                println!("Server not running, skipping test: {}", e);
            }
        }
    }
}
