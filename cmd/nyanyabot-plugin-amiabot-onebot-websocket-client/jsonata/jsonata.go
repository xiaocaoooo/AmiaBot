package jsonata

import (
	"errors"
	"fmt"
	"strconv"
	"strings"
)

// Expr represents a compiled JSONata-compatible expression.
type Expr struct {
	root Node
}

// Compile compiles a JSONata-compatible expression string into an Expr.
func Compile(exprStr string) (*Expr, error) {
	tokens, err := tokenize(exprStr)
	if err != nil {
		return nil, err
	}
	p := &parser{tokens: tokens, pos: 0}
	node, err := p.parseExpression()
	if err != nil {
		return nil, err
	}
	if p.pos < len(p.tokens) {
		return nil, fmt.Errorf("unexpected token at pos %d: %v", p.pos, p.tokens[p.pos])
	}
	return &Expr{root: node}, nil
}

// Eval evaluates the compiled expression against the given JSON-like data.
func (e *Expr) Eval(data interface{}) (interface{}, error) {
	if e.root == nil {
		return nil, nil
	}
	return e.root.Eval(data), nil
}

type tokenType int

const (
	tokIdent tokenType = iota
	tokString
	tokNumber
	tokBool
	tokAnd
	tokOr
	tokNot
	tokEq
	tokNEq
	tokLParen
	tokRParen
)

type token struct {
	typ tokenType
	val string
}

func tokenize(s string) ([]token, error) {
	var tokens []token
	i := 0
	n := len(s)
	for i < n {
		c := s[i]
		if isSpace(c) {
			i++
			continue
		}
		if c == '(' {
			tokens = append(tokens, token{typ: tokLParen, val: "("})
			i++
			continue
		}
		if c == ')' {
			tokens = append(tokens, token{typ: tokRParen, val: ")"})
			i++
			continue
		}
		if c == '=' {
			if i+1 < n && s[i+1] == '=' {
				tokens = append(tokens, token{typ: tokEq, val: "=="})
				i += 2
			} else {
				tokens = append(tokens, token{typ: tokEq, val: "="})
				i++
			}
			continue
		}
		if c == '!' {
			if i+1 < n && s[i+1] == '=' {
				tokens = append(tokens, token{typ: tokNEq, val: "!="})
				i += 2
				continue
			}
			return nil, fmt.Errorf("unexpected character '!' at position %d", i)
		}
		if c == '\'' || c == '"' {
			quote := c
			start := i + 1
			i++
			found := false
			for i < n {
				if s[i] == quote {
					found = true
					break
				}
				i++
			}
			if !found {
				return nil, fmt.Errorf("unclosed string starting at position %d", start-1)
			}
			tokens = append(tokens, token{typ: tokString, val: s[start:i]})
			i++
			continue
		}
		if isDigit(c) {
			start := i
			for i < n && (isDigit(s[i]) || s[i] == '.') {
				i++
			}
			tokens = append(tokens, token{typ: tokNumber, val: s[start:i]})
			continue
		}
		if isIdentChar(c) {
			start := i
			for i < n && isIdentChar(s[i]) {
				i++
			}
			val := s[start:i]
			switch strings.ToLower(val) {
			case "and":
				tokens = append(tokens, token{typ: tokAnd, val: val})
			case "or":
				tokens = append(tokens, token{typ: tokOr, val: val})
			case "not":
				tokens = append(tokens, token{typ: tokNot, val: val})
			case "true", "false":
				tokens = append(tokens, token{typ: tokBool, val: val})
			default:
				tokens = append(tokens, token{typ: tokIdent, val: val})
			}
			continue
		}
		return nil, fmt.Errorf("unexpected character %q at position %d", c, i)
	}
	return tokens, nil
}

func isSpace(c byte) bool {
	return c == ' ' || c == '\t' || c == '\n' || c == '\r'
}

func isDigit(c byte) bool {
	return c >= '0' && c <= '9'
}

func isIdentChar(c byte) bool {
	return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '_' || c == '-' || c == '.'
}

type Node interface {
	Eval(data interface{}) interface{}
}

type ConstNode struct {
	val interface{}
}

func (n *ConstNode) Eval(data interface{}) interface{} {
	return n.val
}

type PathNode struct {
	parts []string
}

func (n *PathNode) Eval(data interface{}) interface{} {
	curr := data
	for _, part := range n.parts {
		if curr == nil {
			return nil
		}
		m, ok := curr.(map[string]interface{})
		if !ok {
			return nil
		}
		curr = m[part]
	}
	return curr
}

type BinaryOpNode struct {
	op    tokenType
	left  Node
	right Node
}

func (n *BinaryOpNode) Eval(data interface{}) interface{} {
	lVal := n.left.Eval(data)
	rVal := n.right.Eval(data)

	switch n.op {
	case tokEq:
		return compareValues(lVal, rVal)
	case tokNEq:
		return !compareValues(lVal, rVal)
	case tokAnd:
		return isTruthy(lVal) && isTruthy(rVal)
	case tokOr:
		return isTruthy(lVal) || isTruthy(rVal)
	}
	return false
}

type UnaryOpNode struct {
	op   tokenType
	expr Node
}

func (n *UnaryOpNode) Eval(data interface{}) interface{} {
	val := n.expr.Eval(data)
	switch n.op {
	case tokNot:
		return !isTruthy(val)
	}
	return false
}

func isTruthy(v interface{}) bool {
	if v == nil {
		return false
	}
	switch val := v.(type) {
	case bool:
		return val
	case string:
		return val != ""
	case int:
		return val != 0
	case int64:
		return val != 0
	case float64:
		return val != 0.0
	}
	return true
}

func compareValues(a, b interface{}) bool {
	if a == nil && b == nil {
		return true
	}
	if a == nil || b == nil {
		return false
	}

	aNum, aIsNum := toFloat64(a)
	bNum, bIsNum := toFloat64(b)
	if aIsNum && bIsNum {
		return aNum == bNum
	}

	return fmt.Sprintf("%v", a) == fmt.Sprintf("%v", b)
}

func toFloat64(v interface{}) (float64, bool) {
	switch val := v.(type) {
	case int:
		return float64(val), true
	case int32:
		return float64(val), true
	case int64:
		return float64(val), true
	case float32:
		return float64(val), true
	case float64:
		return val, true
	case string:
		if f, err := strconv.ParseFloat(val, 64); err == nil {
			return f, true
		}
	}
	return 0, false
}

type parser struct {
	tokens []token
	pos    int
}

func (p *parser) match(types ...tokenType) bool {
	if p.pos >= len(p.tokens) {
		return false
	}
	t := p.tokens[p.pos].typ
	for _, typ := range types {
		if t == typ {
			p.pos++
			return true
		}
	}
	return false
}

func (p *parser) peek() (token, bool) {
	if p.pos >= len(p.tokens) {
		return token{}, false
	}
	return p.tokens[p.pos], true
}

func (p *parser) parseExpression() (Node, error) {
	return p.parseOr()
}

func (p *parser) parseOr() (Node, error) {
	node, err := p.parseAnd()
	if err != nil {
		return nil, err
	}
	for p.match(tokOr) {
		right, err := p.parseAnd()
		if err != nil {
			return nil, err
		}
		node = &BinaryOpNode{op: tokOr, left: node, right: right}
	}
	return node, nil
}

func (p *parser) parseAnd() (Node, error) {
	node, err := p.parseNot()
	if err != nil {
		return nil, err
	}
	for p.match(tokAnd) {
		right, err := p.parseNot()
		if err != nil {
			return nil, err
		}
		node = &BinaryOpNode{op: tokAnd, left: node, right: right}
	}
	return node, nil
}

func (p *parser) parseNot() (Node, error) {
	if p.match(tokNot) {
		expr, err := p.parseNot()
		if err != nil {
			return nil, err
		}
		return &UnaryOpNode{op: tokNot, expr: expr}, nil
	}
	return p.parseComparison()
}

func (p *parser) parseComparison() (Node, error) {
	node, err := p.parsePrimary()
	if err != nil {
		return nil, err
	}
	if p.match(tokEq, tokNEq) {
		op := p.tokens[p.pos-1].typ
		right, err := p.parsePrimary()
		if err != nil {
			return nil, err
		}
		node = &BinaryOpNode{op: op, left: node, right: right}
	}
	return node, nil
}

func (p *parser) parsePrimary() (Node, error) {
	tok, ok := p.peek()
	if !ok {
		return nil, errors.New("unexpected end of expression")
	}
	switch tok.typ {
	case tokLParen:
		p.pos++
		node, err := p.parseExpression()
		if err != nil {
			return nil, err
		}
		if !p.match(tokRParen) {
			return nil, errors.New("unmatched parentheses")
		}
		return node, nil
	case tokString:
		p.pos++
		return &ConstNode{val: tok.val}, nil
	case tokNumber:
		p.pos++
		if f, err := strconv.ParseFloat(tok.val, 64); err == nil {
			return &ConstNode{val: f}, nil
		}
		return &ConstNode{val: tok.val}, nil
	case tokBool:
		p.pos++
		return &ConstNode{val: strings.ToLower(tok.val) == "true"}, nil
	case tokIdent:
		p.pos++
		parts := strings.Split(tok.val, ".")
		return &PathNode{parts: parts}, nil
	}
	return nil, fmt.Errorf("unexpected token %v", tok)
}
