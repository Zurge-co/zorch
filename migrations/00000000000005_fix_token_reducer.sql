-- Fix Token Reducer seed script: escape newlines in split/join separator string.
UPDATE middleware_configs
SET config = jsonb_set(
    config,
    '{source}',
    to_jsonb('fn run(ctx, input, config) {
    let body = input.body;
    if body.contains("messages") {
        for msg in body.messages {
            if msg.contains("content") && type_of(msg.content) == "string" {
                let s = msg.content;
                let lines = s.split("\n");
                let trimmed = [];
                for line in lines {
                    trimmed.push(line.trim());
                }
                s = trimmed.join("\n");
                let parts = s.split(" ");
                let non_empty = [];
                for p in parts {
                    if len(p) > 0 {
                        non_empty.push(p);
                    }
                }
                s = non_empty.join(" ");
                msg.content = s;
            }
        }
    }
    return #{ action: "continue", body: body, metadata: #{ trimmed: true } };
}'::text)
)
WHERE name = 'Token Reducer';
