-- Fix Prompt Injector seed script: escape newlines in injected separator string.
UPDATE middleware_configs
SET config = jsonb_set(
    config,
    '{source}',
    to_jsonb('fn run(ctx, input, config) {
    let text = config.text;
    let position = config.position;
    let body = input.body;
    if !body.contains("messages") {
        body.messages = [];
    }
    let messages = body.messages;
    if position == "system_prefix" {
        if len(messages) > 0 && messages[0].role == "system" && type_of(messages[0].content) == "string" {
            messages[0].content = text + "\\n\\n" + messages[0].content;
        } else {
            messages.insert(0, #{ role: "system", content: text });
        }
        return #{ action: "continue", body: body, metadata: #{ injected: true, position: position } };
    }
    if position == "system_suffix" {
        let found = false;
        for msg in messages {
            if msg.role == "system" && type_of(msg.content) == "string" {
                msg.content = msg.content + "\\n\\n" + text;
                found = true;
                break;
            }
        }
        if !found {
            messages.push(#{ role: "system", content: text });
        }
        return #{ action: "continue", body: body, metadata: #{ injected: true, position: position } };
    }
    return #{ action: "continue", metadata: #{ error: "unknown position" } };
}'::text)
)
WHERE name = 'Prompt Injector';
