package calc

import (
	"errors"
	"math"
	"math/big"
	"strconv"
	"strings"
	"unicode"
	"unicode/utf8"
)

// The calculator reads a small part of fend's language: numbers, + - * / ^
// and %, units and currencies, `to` for conversions, and a few functions. It
// computes with exact fractions, as fend does, so 0.1 + 0.2 is 0.3.

var errNoAnswer = errors.New("no answer")

type tokenKind uint8

const (
	tokenNumber tokenKind = iota
	tokenName
	tokenSign
	tokenEnd
)

type token struct {
	kind tokenKind
	text string
}

// lex splits fend input into numbers, names, and signs.
func lex(input string) ([]token, error) {
	var tokens []token
	for i := 0; i < len(input); {
		r, size := utf8.DecodeRuneInString(input[i:])
		switch {
		case unicode.IsSpace(r):
			i += size
		case isDigit(r) || r == '.' && i+1 < len(input) && isDigit(rune(input[i+1])):
			end := numberEnd(input, i)
			tokens = append(tokens, token{tokenNumber, input[i:end]})
			i = end
		case strings.HasPrefix(input[i:], "**"):
			tokens = append(tokens, token{tokenSign, "^"})
			i += 2
		case strings.ContainsRune("+-*/^()%", r):
			tokens = append(tokens, token{tokenSign, string(r)})
			i += size
		case strings.ContainsRune("$£¥€₺", r):
			tokens = append(tokens, token{tokenName, string(r)})
			i += size
		case unicode.IsLetter(r) || r == '_' || r == '°':
			end := nameEnd(input, i+size)
			// `US$` and the other dollars, and fend's `approx.`.
			switch {
			case strings.HasPrefix(input[end:], "$") && isCurrencyName(input[i:end+1]):
				end++
			case input[i:end] == "approx" && strings.HasPrefix(input[end:], "."):
				end++
			}
			tokens = append(tokens, token{tokenName, input[i:end]})
			i = end
		default:
			return nil, errNoAnswer
		}
	}
	return append(tokens, token{kind: tokenEnd}), nil
}

func isDigit(r rune) bool { return r >= '0' && r <= '9' }

// numberEnd is where the number at start ends: digits with commas between
// them, a decimal part, and an exponent, as in `1,000.5` or `2e-3`.
func numberEnd(input string, start int) int {
	i := start
	digits := func() {
		for i < len(input) && (isDigit(rune(input[i])) ||
			input[i] == ',' && i > start && isDigit(rune(input[i-1])) && i+1 < len(input) && isDigit(rune(input[i+1]))) {
			i++
		}
	}
	digits()
	if i < len(input) && input[i] == '.' {
		i++
		digits()
	}
	if i < len(input) && (input[i] == 'e' || input[i] == 'E') {
		j := i + 1
		if j < len(input) && (input[j] == '+' || input[j] == '-') {
			j++
		}
		if j < len(input) && isDigit(rune(input[j])) {
			i = j
			for i < len(input) && isDigit(rune(input[i])) {
				i++
			}
		}
	}
	return i
}

func nameEnd(input string, i int) int {
	for i < len(input) {
		r, size := utf8.DecodeRuneInString(input[i:])
		if !unicode.IsLetter(r) && !unicode.IsDigit(r) && r != '_' {
			break
		}
		i += size
	}
	return i
}

func isCurrencyName(name string) bool {
	for _, c := range currencyNames {
		if c.singular == name {
			return true
		}
	}
	return false
}

// The parse tree. Evaluating it is a second step, because a conversion
// evaluates its target first, as fend does: `100 usd to qqq` fails before
// it asks for the dollar's rate.
type node any

type (
	numberNode struct{ text string }
	nameNode   struct{ name string }
	negateNode struct{ x node }
	// percentNode is `x%`.
	percentNode struct{ x node }
	binaryNode  struct {
		// One of + - * / ^ mod, and "mixed" for a mixed fraction such
		// as `401 83/117`.
		op   string
		x, y node
	}
	callNode struct {
		function string
		argument node
	}
	convertNode struct{ x, target node }
	// percentTarget is the `%` in `0.35 to %`.
	percentTarget struct{}
)

var functions = map[string]bool{"sqrt": true, "abs": true, "floor": true, "ceil": true, "round": true, "approx.": true, "approximately": true}

// operatorWords are fend's words that work as signs. Only lowercase:
// fend reads `Mod` as a name.
var operatorWords = map[string]string{"to": "to", "as": "to", "in": "to", "per": "/", "mod": "mod"}

type parser struct {
	tokens []token
	at     int
}

func parse(input string) (node, error) {
	tokens, err := lex(input)
	if err != nil {
		return nil, err
	}
	p := &parser{tokens: tokens}
	n, err := p.conversion()
	if err != nil {
		return nil, err
	}
	if p.peek().kind != tokenEnd {
		return nil, errNoAnswer
	}
	return n, nil
}

func (p *parser) peek() token { return p.tokens[p.at] }

func (p *parser) next() token {
	t := p.tokens[p.at]
	if t.kind != tokenEnd {
		p.at++
	}
	return t
}

func (p *parser) isSign(text string) bool {
	t := p.peek()
	return t.kind == tokenSign && t.text == text
}

// operator is the operator word or sign at the parser, or "".
func (p *parser) operator() string {
	t := p.peek()
	switch t.kind {
	case tokenSign:
		return t.text
	case tokenName:
		return operatorWords[t.text]
	}
	return ""
}

// remainder reports whether the `%` at the parser is a remainder, as fend
// reads one before a number or `(`, rather than a percentage.
func (p *parser) remainder() bool {
	if !p.isSign("%") {
		return false
	}
	after := p.tokens[p.at+1]
	return after.kind == tokenNumber || after.kind == tokenSign && after.text == "("
}

func (p *parser) conversion() (node, error) {
	x, err := p.additive()
	for err == nil && p.operator() == "to" {
		p.next()
		var target node
		if p.isSign("%") {
			p.next()
			target = percentTarget{}
		} else if target, err = p.juxtaposition(); err != nil {
			return nil, err
		}
		x = convertNode{x, target}
	}
	return x, err
}

func (p *parser) additive() (node, error) {
	x, err := p.juxtaposition()
	for err == nil && (p.isSign("+") || p.isSign("-")) {
		op := p.next().text
		var y node
		if y, err = p.juxtaposition(); err == nil {
			x = binaryNode{op, x, y}
		}
	}
	return x, err
}

// juxtaposition is values side by side, as in `5 kg` or `20k rmb`. It binds
// less tightly than `*` and `/`, so `47000/117 MYR` is (47000/117) MYR.
func (p *parser) juxtaposition() (node, error) {
	x, err := p.multiplicative()
	for err == nil && p.startsOperand() {
		var y node
		if y, err = p.multiplicative(); err == nil {
			x = binaryNode{juxtaposed(x, y), x, y}
		}
	}
	return x, err
}

// juxtaposed is "mixed" for a mixed fraction, such as `401 83/117`, where
// fend adds, and "*" for anything else.
func juxtaposed(x, y node) string {
	whole, ok := x.(numberNode)
	fraction, isDivision := y.(binaryNode)
	if !ok || strings.ContainsAny(whole.text, ".eE") || !isDivision || fraction.op != "/" {
		return "*"
	}
	_, top := fraction.x.(numberNode)
	_, bottom := fraction.y.(numberNode)
	if top && bottom {
		return "mixed"
	}
	return "*"
}

func (p *parser) startsOperand() bool {
	t := p.peek()
	switch t.kind {
	case tokenNumber:
		return true
	case tokenName:
		return operatorWords[t.text] == ""
	case tokenSign:
		return t.text == "("
	}
	return false
}

func (p *parser) multiplicative() (node, error) {
	x, err := p.unary()
	for err == nil {
		op := p.operator()
		if op == "%" && p.remainder() {
			op = "mod"
		}
		if op != "*" && op != "/" && op != "mod" {
			break
		}
		p.next()
		var y node
		if y, err = p.unary(); err == nil {
			x = binaryNode{op, x, y}
		}
	}
	return x, err
}

func (p *parser) unary() (node, error) {
	switch {
	case p.isSign("-"):
		p.next()
		x, err := p.unary()
		return negateNode{x}, err
	case p.isSign("+"):
		p.next()
		return p.unary()
	}
	return p.power()
}

func (p *parser) power() (node, error) {
	x, err := p.postfix()
	if err != nil || !p.isSign("^") {
		return x, err
	}
	p.next()
	y, err := p.unary()
	return binaryNode{"^", x, y}, err
}

func (p *parser) postfix() (node, error) {
	x, err := p.primary()
	for err == nil && p.isSign("%") && !p.remainder() {
		p.next()
		x = percentNode{x}
	}
	return x, err
}

func (p *parser) primary() (node, error) {
	t := p.next()
	switch {
	case t.kind == tokenNumber:
		return numberNode{t.text}, nil
	case t.kind == tokenSign && t.text == "(":
		x, err := p.conversion()
		if err != nil {
			return nil, err
		}
		if !p.isSign(")") {
			return nil, errNoAnswer
		}
		p.next()
		return x, nil
	case t.kind == tokenName && functions[t.text]:
		argument, err := p.postfix()
		return callNode{t.text, argument}, err
	case t.kind == tokenName && operatorWords[t.text] == "":
		return nameNode{t.text}, nil
	}
	return nil, errNoAnswer
}

// style is how a value is written: as fend chooses, or as `to exact` or
// `to fraction` asks.
type style uint8

const (
	styleAuto style = iota
	styleExact
	styleFraction
)

type component struct {
	unit unit
	exp  int
}

// quantity is a value with its units.
type quantity struct {
	value *big.Rat
	// inexact marks a value that is rounded, such as pi or a square root.
	inexact bool
	units   []component
	style   style
}

// Limits that keep a calculation fast: search runs on every keystroke. fend
// stops at a deadline instead.
const (
	// maxExponent bounds the exponent of a number, such as 1e1000, and a
	// whole power, such as 2^1000.
	maxExponent = 1000
	// maxBits bounds the size of every value, about 20,000 digits.
	maxBits = 1 << 16
)

func plain(v *big.Rat) quantity { return quantity{value: v} }

// evaluate evaluates n, and fails for a value too large to write fast.
func evaluate(n node, rates *rateSource) (quantity, error) {
	q, err := evaluateNode(n, rates)
	if err == nil && q.value.Num().BitLen()+q.value.Denom().BitLen() > maxBits {
		return quantity{}, errNoAnswer
	}
	return q, err
}

func evaluateNode(n node, rates *rateSource) (quantity, error) {
	switch n := n.(type) {
	case numberNode:
		return number(n.text)
	case nameNode:
		return name(n.name, rates)
	case negateNode:
		x, err := evaluate(n.x, rates)
		if err != nil {
			return quantity{}, err
		}
		x.value = new(big.Rat).Neg(x.value)
		return x, nil
	case percentNode:
		x, err := evaluate(n.x, rates)
		if err != nil {
			return quantity{}, err
		}
		return multiply(x, quantity{value: big.NewRat(1, 1), units: []component{{percentUnit, 1}}})
	case binaryNode:
		x, err := evaluate(n.x, rates)
		if err != nil {
			return quantity{}, err
		}
		y, err := evaluate(n.y, rates)
		if err != nil {
			return quantity{}, err
		}
		return binary(n.op, x, y)
	case callNode:
		x, err := evaluate(n.argument, rates)
		if err != nil {
			return quantity{}, err
		}
		return call(n.function, x)
	case convertNode:
		return convertTo(n, rates)
	}
	return quantity{}, errNoAnswer
}

func number(text string) (quantity, error) {
	text = strings.ReplaceAll(text, ",", "")
	if _, exponent, ok := strings.Cut(strings.ToLower(text), "e"); ok {
		if e, err := strconv.Atoi(exponent); err != nil || e > maxExponent || e < -maxExponent {
			return quantity{}, errNoAnswer
		}
	}
	v, ok := new(big.Rat).SetString(text)
	if !ok {
		return quantity{}, errNoAnswer
	}
	return plain(v), nil
}

// pi and e as fend writes them when rounded: pi to more digits than it
// shows, and e as fend defines it.
var (
	pi    = rat("3.14159265358979323846264338327950288419716939937510582097494459")
	euler = rat("2.718281828459045235")
)

func name(text string, rates *rateSource) (quantity, error) {
	switch text {
	case "pi", "π":
		return quantity{value: new(big.Rat).Set(pi), inexact: true}, nil
	case "e":
		return quantity{value: new(big.Rat).Set(euler), inexact: true}, nil
	}
	if v, ok := numberWords[text]; ok {
		return plain(rat(v)), nil
	}
	u, ok, err := lookupUnit(text, rates)
	if err != nil || !ok {
		return quantity{}, errNoAnswer
	}
	return quantity{value: big.NewRat(1, 1), units: []component{{u, 1}}}, nil
}

func binary(op string, x, y quantity) (quantity, error) {
	switch op {
	case "+", "mixed":
		return add(x, y)
	case "-":
		y.value = new(big.Rat).Neg(y.value)
		return add(x, y)
	case "*":
		return multiply(x, y)
	case "/":
		if y.value.Sign() == 0 {
			return quantity{}, errNoAnswer
		}
		y.value = new(big.Rat).Inv(y.value)
		y.units = invert(y.units)
		return multiply(x, y)
	case "mod":
		// fend takes a remainder only of whole numbers of 0 or more.
		if len(x.units) > 0 || len(y.units) > 0 || !x.value.IsInt() || !y.value.IsInt() ||
			x.value.Sign() < 0 || y.value.Sign() <= 0 {
			return quantity{}, errNoAnswer
		}
		x.value = new(big.Rat).SetInt(new(big.Int).Mod(x.value.Num(), y.value.Num()))
		x.inexact = x.inexact || y.inexact
		return x, nil
	case "^":
		return power(x, y)
	}
	return quantity{}, errNoAnswer
}

func invert(units []component) []component {
	out := make([]component, len(units))
	for i, c := range units {
		out[i] = component{c.unit, -c.exp}
	}
	return out
}

func (q quantity) dims() dims {
	var d dims
	for _, c := range q.units {
		for i, n := range c.unit.dims {
			d[i] += n * c.exp
		}
	}
	return d
}

// scale is the size of the units in base units.
func (q quantity) scale() *big.Rat {
	s := big.NewRat(1, 1)
	for _, c := range q.units {
		s.Mul(s, ratPower(c.unit.scale, c.exp))
	}
	return s
}

func ratPower(r *big.Rat, n int) *big.Rat {
	out := big.NewRat(1, 1)
	base := new(big.Rat).Set(r)
	if n < 0 {
		base.Inv(base)
		n = -n
	}
	num := new(big.Int).Exp(base.Num(), big.NewInt(int64(n)), nil)
	den := new(big.Int).Exp(base.Denom(), big.NewInt(int64(n)), nil)
	return out.SetFrac(num, den)
}

// add adds y in the units of x.
func add(x, y quantity) (quantity, error) {
	if x.dims() != y.dims() {
		return quantity{}, errNoAnswer
	}
	v := new(big.Rat).Mul(y.value, y.scale())
	v.Quo(v, x.scale())
	x.value = new(big.Rat).Add(x.value, v)
	x.inexact = x.inexact || y.inexact
	return x, nil
}

// multiply keeps the units of both, merging one that measures the same as
// one before it: `m * cm` is m^2.
func multiply(x, y quantity) (quantity, error) {
	out := quantity{
		value:   new(big.Rat).Mul(x.value, y.value),
		inexact: x.inexact || y.inexact,
		units:   append([]component(nil), x.units...),
		style:   x.style,
	}
next:
	for _, c := range y.units {
		for i, have := range out.units {
			same := have.unit.singular == c.unit.singular
			mergeable := c.unit.dims != (dims{}) && have.unit.dims == c.unit.dims &&
				have.unit.offset == nil && c.unit.offset == nil
			if same || mergeable {
				ratio := new(big.Rat).Quo(c.unit.scale, have.unit.scale)
				out.value.Mul(out.value, ratPower(ratio, c.exp))
				out.units[i].exp += c.exp
				continue next
			}
		}
		out.units = append(out.units, c)
	}
	kept := out.units[:0]
	for _, c := range out.units {
		// Money squared is not an amount of money.
		if c.unit.currency != "" && (c.exp > 1 || c.exp < -1) {
			return quantity{}, errNoAnswer
		}
		if c.exp != 0 {
			kept = append(kept, c)
		}
	}
	out.units = kept
	return simplifyPercent(out), nil
}

// simplifyPercent keeps a percentage only alone, as fend does: `5% * 2` is
// 10%, but a percentage of a unit is a plain factor.
func simplifyPercent(q quantity) quantity {
	for i, c := range q.units {
		if !c.unit.percent {
			continue
		}
		if len(q.units) == 1 && c.exp > 0 {
			q.value.Mul(q.value, ratPower(c.unit.scale, c.exp-1))
			q.units = []component{{c.unit, 1}}
			return q
		}
		q.value.Mul(q.value, ratPower(c.unit.scale, c.exp))
		q.units = append(q.units[:i:i], q.units[i+1:]...)
		return q
	}
	return q
}

func power(x, y quantity) (quantity, error) {
	if len(y.units) > 0 {
		return quantity{}, errNoAnswer
	}
	if y.value.IsInt() {
		if !y.value.Num().IsInt64() {
			return quantity{}, errNoAnswer
		}
		n := y.value.Num().Int64()
		bits := int64(x.value.Num().BitLen() + x.value.Denom().BitLen())
		if n > maxExponent || n < -maxExponent || bits*abs(n) > maxBits {
			return quantity{}, errNoAnswer
		}
		if n < 0 && x.value.Sign() == 0 {
			return quantity{}, errNoAnswer
		}
		out := quantity{value: ratPower(x.value, int(n)), inexact: x.inexact || y.inexact, style: x.style}
		for _, c := range x.units {
			out.units = append(out.units, component{c.unit, c.exp * int(n)})
		}
		return simplifyPercent(out), nil
	}
	if len(x.units) > 0 || x.value.Sign() < 0 {
		return quantity{}, errNoAnswer
	}
	if y.value.Denom().IsInt64() && y.value.Denom().Int64() == 2 {
		root, err := squareRoot(x)
		if err != nil {
			return quantity{}, err
		}
		return power(root, plain(new(big.Rat).SetInt(y.value.Num())))
	}
	base, _ := x.value.Float64()
	exponent, _ := y.value.Float64()
	return fromFloat(math.Pow(base, exponent))
}

func abs(n int64) int64 {
	if n < 0 {
		return -n
	}
	return n
}

// fromFloat is a rounded value from a float64, as its shortest decimal.
func fromFloat(f float64) (quantity, error) {
	if math.IsNaN(f) || math.IsInf(f, 0) {
		return quantity{}, errNoAnswer
	}
	q := plain(rat(strconv.FormatFloat(f, 'g', -1, 64)))
	q.inexact = true
	return q, nil
}

// squareRoot is exact for a square, such as 144 or 9/4, and rounded for
// anything else. Units must be squares too: sqrt(4 m^2) is 2 m.
func squareRoot(x quantity) (quantity, error) {
	if x.value.Sign() < 0 {
		return quantity{}, errNoAnswer
	}
	out := quantity{inexact: x.inexact, style: x.style}
	for _, c := range x.units {
		if c.exp%2 != 0 {
			return quantity{}, errNoAnswer
		}
		out.units = append(out.units, component{c.unit, c.exp / 2})
	}
	num, den := new(big.Int).Sqrt(x.value.Num()), new(big.Int).Sqrt(x.value.Denom())
	if new(big.Int).Mul(num, num).Cmp(x.value.Num()) == 0 && new(big.Int).Mul(den, den).Cmp(x.value.Denom()) == 0 {
		out.value = new(big.Rat).SetFrac(num, den)
		return out, nil
	}
	root := new(big.Float).SetPrec(256).SetRat(x.value)
	root.Sqrt(root)
	out.value, _ = root.Rat(nil)
	out.inexact = true
	return out, nil
}

func call(function string, x quantity) (quantity, error) {
	v := x.value
	switch function {
	case "sqrt":
		return squareRoot(x)
	case "abs":
		x.value = new(big.Rat).Abs(v)
	case "floor", "ceil", "round":
		x.value = new(big.Rat).SetInt(whole(function, v))
	case "approx.", "approximately":
		x.inexact = true
	}
	return x, nil
}

// whole rounds down, up, or to the nearest, halves away from zero.
func whole(how string, v *big.Rat) *big.Int {
	q, m := new(big.Int).DivMod(v.Num(), v.Denom(), new(big.Int)) // q rounds down: m >= 0
	switch how {
	case "ceil":
		if m.Sign() != 0 {
			q.Add(q, big.NewInt(1))
		}
	case "round":
		twice := new(big.Int).Lsh(m, 1)
		c := twice.Cmp(v.Denom())
		if c > 0 || c == 0 && v.Sign() > 0 {
			q.Add(q, big.NewInt(1))
		}
	}
	return q
}

func convertTo(n convertNode, rates *rateSource) (quantity, error) {
	var target quantity
	switch t := n.target.(type) {
	case percentTarget:
		target = quantity{value: big.NewRat(1, 1), units: []component{{percentUnit, 1}}}
	case nameNode:
		if t.name == "exact" || t.name == "fraction" {
			x, err := evaluate(n.x, rates)
			if t.name == "exact" {
				x.style = styleExact
			} else {
				x.style = styleFraction
			}
			return x, err
		}
	}
	if target.value == nil {
		var err error
		if target, err = evaluate(n.target, rates); err != nil {
			return quantity{}, err
		}
	}
	x, err := evaluate(n.x, rates)
	if err != nil {
		return quantity{}, err
	}
	if target.value.Cmp(big.NewRat(1, 1)) != 0 || x.dims() != target.dims() {
		return quantity{}, errNoAnswer
	}
	out := quantity{inexact: x.inexact || target.inexact, units: target.units, style: x.style}
	if from, to, ok := temperatures(x, target); ok {
		kelvin := new(big.Rat).Mul(x.value, from.scale)
		kelvin.Add(kelvin, offset(from))
		kelvin.Sub(kelvin, offset(to))
		out.value = kelvin.Quo(kelvin, to.scale)
		return out, nil
	}
	out.value = new(big.Rat).Mul(x.value, x.scale())
	out.value.Quo(out.value, target.scale())
	return out, nil
}

// temperatures returns the two scales of a conversion from one temperature
// to another, where their zeros differ.
func temperatures(x, target quantity) (from, to unit, ok bool) {
	if len(x.units) != 1 || len(target.units) != 1 || x.units[0].exp != 1 || target.units[0].exp != 1 {
		return unit{}, unit{}, false
	}
	from, to = x.units[0].unit, target.units[0].unit
	return from, to, from.dims[dimTemperature] == 1 && (from.offset != nil || to.offset != nil)
}

func offset(u unit) *big.Rat {
	if u.offset == nil {
		return new(big.Rat)
	}
	return u.offset
}

// rateSource answers the calculator's questions for exchange rates and
// notes them, as fend's exchange rate handler does.
type rateSource struct {
	rates Rates
	// probe answers 1 for every currency: it only finds whether a query
	// needs rates.
	probe bool
	// asked: a rate was asked for.
	asked bool
	// missing is the first currency asked for that rates do not have.
	missing string
}

var errNoRate = errors.New("no exchange rate")

// euros is the value of one unit of code in euros.
func (s *rateSource) euros(code string) (*big.Rat, error) {
	if s.probe {
		s.asked = true
		return big.NewRat(1, 1), nil
	}
	if s.rates == nil {
		return nil, errNoRate
	}
	s.asked = true
	perEuro, ok := s.rates.PerEuro(code)
	if !ok || perEuro <= 0 {
		if s.missing == "" {
			s.missing = code
		}
		return nil, errNoRate
	}
	// A rate as its shortest decimal, so 1.16 is 116/100: fend's exact
	// fractions of money, such as `422 12/29 MYR`, depend on it.
	return new(big.Rat).Inv(rat(strconv.FormatFloat(perEuro, 'g', -1, 64))), nil
}
