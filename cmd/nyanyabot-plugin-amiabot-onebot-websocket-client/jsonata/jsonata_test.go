package jsonata

import (
	"testing"
)

func TestCompileAndEval(t *testing.T) {
	tests := []struct {
		expr    string
		data    interface{}
		wantVal interface{}
		wantErr bool
	}{
		{
			expr: "post_type = 'message'",
			data: map[string]interface{}{
				"post_type": "message",
			},
			wantVal: true,
		},
		{
			expr: "post_type = 'message' and message_type = 'group'",
			data: map[string]interface{}{
				"post_type":    "message",
				"message_type": "group",
			},
			wantVal: true,
		},
		{
			expr: "post_type = 'message' and message_type = 'group'",
			data: map[string]interface{}{
				"post_type":    "message",
				"message_type": "private",
			},
			wantVal: false,
		},
		{
			expr: "sender.user_id = 12345",
			data: map[string]interface{}{
				"sender": map[string]interface{}{
					"user_id": 12345,
				},
			},
			wantVal: true,
		},
		{
			expr: "sender.user_id = 12345",
			data: map[string]interface{}{
				"sender": map[string]interface{}{
					"user_id": 12345.0, // float64 from json
				},
			},
			wantVal: true,
		},
		{
			expr: "not (action = 'send_msg')",
			data: map[string]interface{}{
				"action": "get_status",
			},
			wantVal: true,
		},
	}

	for _, tt := range tests {
		expr, err := Compile(tt.expr)
		if (err != nil) != tt.wantErr {
			t.Fatalf("Compile(%q) error = %v, wantErr %v", tt.expr, err, tt.wantErr)
		}
		if err != nil {
			continue
		}
		got, err := expr.Eval(tt.data)
		if err != nil {
			t.Fatalf("Eval error = %v", err)
		}
		if got != tt.wantVal {
			t.Errorf("Eval(%q) = %v, want %v", tt.expr, got, tt.wantVal)
		}
	}
}
