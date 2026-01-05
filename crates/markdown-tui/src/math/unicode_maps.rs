//! Unicode character mappings for mathematical symbols

use once_cell::sync::Lazy;
use std::collections::HashMap;

/// Unicode superscript characters
pub static SUPERSCRIPTS: Lazy<HashMap<char, char>> = Lazy::new(|| {
    [
        ('0', '⁰'),
        ('1', '¹'),
        ('2', '²'),
        ('3', '³'),
        ('4', '⁴'),
        ('5', '⁵'),
        ('6', '⁶'),
        ('7', '⁷'),
        ('8', '⁸'),
        ('9', '⁹'),
        ('+', '⁺'),
        ('-', '⁻'),
        ('=', '⁼'),
        ('(', '⁽'),
        (')', '⁾'),
        ('a', 'ᵃ'),
        ('b', 'ᵇ'),
        ('c', 'ᶜ'),
        ('d', 'ᵈ'),
        ('e', 'ᵉ'),
        ('f', 'ᶠ'),
        ('g', 'ᵍ'),
        ('h', 'ʰ'),
        ('i', 'ⁱ'),
        ('j', 'ʲ'),
        ('k', 'ᵏ'),
        ('l', 'ˡ'),
        ('m', 'ᵐ'),
        ('n', 'ⁿ'),
        ('o', 'ᵒ'),
        ('p', 'ᵖ'),
        ('r', 'ʳ'),
        ('s', 'ˢ'),
        ('t', 'ᵗ'),
        ('u', 'ᵘ'),
        ('v', 'ᵛ'),
        ('w', 'ʷ'),
        ('x', 'ˣ'),
        ('y', 'ʸ'),
        ('z', 'ᶻ'),
        (' ', ' '),
    ]
    .iter()
    .copied()
    .collect()
});

/// Unicode subscript characters
pub static SUBSCRIPTS: Lazy<HashMap<char, char>> = Lazy::new(|| {
    [
        ('0', '₀'),
        ('1', '₁'),
        ('2', '₂'),
        ('3', '₃'),
        ('4', '₄'),
        ('5', '₅'),
        ('6', '₆'),
        ('7', '₇'),
        ('8', '₈'),
        ('9', '₉'),
        ('+', '₊'),
        ('-', '₋'),
        ('=', '₌'),
        ('(', '₍'),
        (')', '₎'),
        ('a', 'ₐ'),
        ('e', 'ₑ'),
        ('h', 'ₕ'),
        ('i', 'ᵢ'),
        ('j', 'ⱼ'),
        ('k', 'ₖ'),
        ('l', 'ₗ'),
        ('m', 'ₘ'),
        ('n', 'ₙ'),
        ('o', 'ₒ'),
        ('p', 'ₚ'),
        ('r', 'ᵣ'),
        ('s', 'ₛ'),
        ('t', 'ₜ'),
        ('u', 'ᵤ'),
        ('v', 'ᵥ'),
        ('x', 'ₓ'),
        (' ', ' '),
    ]
    .iter()
    .copied()
    .collect()
});

/// Greek letter mappings (LaTeX name to Unicode)
pub static GREEK_LETTERS: Lazy<HashMap<&'static str, char>> = Lazy::new(|| {
    [
        // Lowercase
        ("alpha", 'α'),
        ("beta", 'β'),
        ("gamma", 'γ'),
        ("delta", 'δ'),
        ("epsilon", 'ε'),
        ("varepsilon", 'ε'),
        ("zeta", 'ζ'),
        ("eta", 'η'),
        ("theta", 'θ'),
        ("vartheta", 'ϑ'),
        ("iota", 'ι'),
        ("kappa", 'κ'),
        ("lambda", 'λ'),
        ("mu", 'μ'),
        ("nu", 'ν'),
        ("xi", 'ξ'),
        ("omicron", 'ο'),
        ("pi", 'π'),
        ("varpi", 'ϖ'),
        ("rho", 'ρ'),
        ("varrho", 'ϱ'),
        ("sigma", 'σ'),
        ("varsigma", 'ς'),
        ("tau", 'τ'),
        ("upsilon", 'υ'),
        ("phi", 'φ'),
        ("varphi", 'ϕ'),
        ("chi", 'χ'),
        ("psi", 'ψ'),
        ("omega", 'ω'),
        // Uppercase
        ("Alpha", 'Α'),
        ("Beta", 'Β'),
        ("Gamma", 'Γ'),
        ("Delta", 'Δ'),
        ("Epsilon", 'Ε'),
        ("Zeta", 'Ζ'),
        ("Eta", 'Η'),
        ("Theta", 'Θ'),
        ("Iota", 'Ι'),
        ("Kappa", 'Κ'),
        ("Lambda", 'Λ'),
        ("Mu", 'Μ'),
        ("Nu", 'Ν'),
        ("Xi", 'Ξ'),
        ("Omicron", 'Ο'),
        ("Pi", 'Π'),
        ("Rho", 'Ρ'),
        ("Sigma", 'Σ'),
        ("Tau", 'Τ'),
        ("Upsilon", 'Υ'),
        ("Phi", 'Φ'),
        ("Chi", 'Χ'),
        ("Psi", 'Ψ'),
        ("Omega", 'Ω'),
    ]
    .iter()
    .copied()
    .collect()
});

/// Mathematical operators and symbols
pub static MATH_SYMBOLS: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    [
        // Binary operators
        ("pm", "±"),
        ("mp", "∓"),
        ("times", "×"),
        ("div", "÷"),
        ("cdot", "·"),
        ("ast", "∗"),
        ("star", "⋆"),
        ("circ", "∘"),
        ("bullet", "•"),
        ("oplus", "⊕"),
        ("ominus", "⊖"),
        ("otimes", "⊗"),
        ("oslash", "⊘"),
        ("odot", "⊙"),
        // Relations
        ("leq", "≤"),
        ("le", "≤"),
        ("geq", "≥"),
        ("ge", "≥"),
        ("neq", "≠"),
        ("ne", "≠"),
        ("equiv", "≡"),
        ("approx", "≈"),
        ("cong", "≅"),
        ("sim", "∼"),
        ("simeq", "≃"),
        ("propto", "∝"),
        ("ll", "≪"),
        ("gg", "≫"),
        ("subset", "⊂"),
        ("supset", "⊃"),
        ("subseteq", "⊆"),
        ("supseteq", "⊇"),
        ("in", "∈"),
        ("notin", "∉"),
        ("ni", "∋"),
        ("perp", "⊥"),
        ("parallel", "∥"),
        // Arrows
        ("leftarrow", "←"),
        ("rightarrow", "→"),
        ("uparrow", "↑"),
        ("downarrow", "↓"),
        ("leftrightarrow", "↔"),
        ("Leftarrow", "⇐"),
        ("Rightarrow", "⇒"),
        ("Uparrow", "⇑"),
        ("Downarrow", "⇓"),
        ("Leftrightarrow", "⇔"),
        ("mapsto", "↦"),
        ("to", "→"),
        ("gets", "←"),
        ("implies", "⟹"),
        ("iff", "⟺"),
        // Big operators
        ("sum", "∑"),
        ("prod", "∏"),
        ("coprod", "∐"),
        ("int", "∫"),
        ("iint", "∬"),
        ("iiint", "∭"),
        ("oint", "∮"),
        ("bigcup", "⋃"),
        ("bigcap", "⋂"),
        ("bigvee", "⋁"),
        ("bigwedge", "⋀"),
        ("bigoplus", "⨁"),
        ("bigotimes", "⨂"),
        // Misc symbols
        ("infty", "∞"),
        ("nabla", "∇"),
        ("partial", "∂"),
        ("forall", "∀"),
        ("exists", "∃"),
        ("nexists", "∄"),
        ("emptyset", "∅"),
        ("varnothing", "∅"),
        ("neg", "¬"),
        ("lnot", "¬"),
        ("land", "∧"),
        ("lor", "∨"),
        ("wedge", "∧"),
        ("vee", "∨"),
        ("cap", "∩"),
        ("cup", "∪"),
        ("setminus", "∖"),
        ("sqrt", "√"),
        ("surd", "√"),
        ("angle", "∠"),
        ("measuredangle", "∡"),
        ("triangle", "△"),
        ("therefore", "∴"),
        ("because", "∵"),
        ("ldots", "…"),
        ("cdots", "⋯"),
        ("vdots", "⋮"),
        ("ddots", "⋱"),
        ("prime", "′"),
        ("dprime", "″"),
        // Delimiters
        ("langle", "⟨"),
        ("rangle", "⟩"),
        ("lceil", "⌈"),
        ("rceil", "⌉"),
        ("lfloor", "⌊"),
        ("rfloor", "⌋"),
        ("lbrace", "{"),
        ("rbrace", "}"),
        ("lvert", "|"),
        ("rvert", "|"),
        ("lVert", "‖"),
        ("rVert", "‖"),
        // Functions (rendered as text)
        ("sin", "sin"),
        ("cos", "cos"),
        ("tan", "tan"),
        ("cot", "cot"),
        ("sec", "sec"),
        ("csc", "csc"),
        ("arcsin", "arcsin"),
        ("arccos", "arccos"),
        ("arctan", "arctan"),
        ("sinh", "sinh"),
        ("cosh", "cosh"),
        ("tanh", "tanh"),
        ("log", "log"),
        ("ln", "ln"),
        ("lg", "lg"),
        ("exp", "exp"),
        ("lim", "lim"),
        ("limsup", "lim sup"),
        ("liminf", "lim inf"),
        ("max", "max"),
        ("min", "min"),
        ("sup", "sup"),
        ("inf", "inf"),
        ("det", "det"),
        ("dim", "dim"),
        ("ker", "ker"),
        ("hom", "hom"),
        ("arg", "arg"),
        ("deg", "deg"),
        ("gcd", "gcd"),
        ("lcm", "lcm"),
        ("mod", "mod"),
        ("Pr", "Pr"),
        // Accents/decorations handled separately
        ("Re", "ℜ"),
        ("Im", "ℑ"),
        ("wp", "℘"),
        ("ell", "ℓ"),
        ("hbar", "ℏ"),
        ("aleph", "ℵ"),
        ("beth", "ℶ"),
        ("gimel", "ℷ"),
        ("daleth", "ℸ"),
    ]
    .iter()
    .copied()
    .collect()
});

/// Try to convert a string to Unicode superscript
pub fn to_superscript(text: &str) -> Option<String> {
    let mut result = String::new();
    for ch in text.chars() {
        if let Some(&sup) = SUPERSCRIPTS.get(&ch) {
            result.push(sup);
        } else {
            return None;
        }
    }
    Some(result)
}

/// Try to convert a string to Unicode subscript
pub fn to_subscript(text: &str) -> Option<String> {
    let mut result = String::new();
    for ch in text.chars() {
        if let Some(&sub) = SUBSCRIPTS.get(&ch) {
            result.push(sub);
        } else {
            return None;
        }
    }
    Some(result)
}

/// Get a math symbol by its LaTeX command name
pub fn get_symbol(name: &str) -> Option<&'static str> {
    MATH_SYMBOLS.get(name).copied()
}

/// Get a Greek letter by its LaTeX name
pub fn get_greek(name: &str) -> Option<char> {
    GREEK_LETTERS.get(name).copied()
}

/// Bracket scaling characters
pub static BRACKETS: Lazy<BracketChars> = Lazy::new(|| BracketChars {
    left_paren: ['⎛', '⎜', '⎝', '('],
    right_paren: ['⎞', '⎟', '⎠', ')'],
    left_bracket: ['⎡', '⎢', '⎣', '['],
    right_bracket: ['⎤', '⎥', '⎦', ']'],
    left_brace: ['⎧', '⎨', '⎩', '{'],
    right_brace: ['⎫', '⎬', '⎭', '}'],
    left_vert: ['│', '│', '│', '|'],
    right_vert: ['│', '│', '│', '|'],
});

pub struct BracketChars {
    pub left_paren: [char; 4], // top, middle, bottom, single
    pub right_paren: [char; 4],
    pub left_bracket: [char; 4],
    pub right_bracket: [char; 4],
    pub left_brace: [char; 4],
    pub right_brace: [char; 4],
    pub left_vert: [char; 4],
    pub right_vert: [char; 4],
}

impl BracketChars {
    pub fn get_left(&self, bracket: &str, height: usize) -> Vec<char> {
        let chars = match bracket {
            "(" | "\\left(" => &self.left_paren,
            "[" | "\\left[" => &self.left_bracket,
            "{" | "\\left{" | "\\left\\{" | "\\lbrace" => &self.left_brace,
            "|" | "\\left|" | "\\lvert" => &self.left_vert,
            _ => &self.left_paren,
        };
        self.scale_bracket(chars, height)
    }

    pub fn get_right(&self, bracket: &str, height: usize) -> Vec<char> {
        let chars = match bracket {
            ")" | "\\right)" => &self.right_paren,
            "]" | "\\right]" => &self.right_bracket,
            "}" | "\\right}" | "\\right\\}" | "\\rbrace" => &self.right_brace,
            "|" | "\\right|" | "\\rvert" => &self.right_vert,
            _ => &self.right_paren,
        };
        self.scale_bracket(chars, height)
    }

    fn scale_bracket(&self, chars: &[char; 4], height: usize) -> Vec<char> {
        if height <= 1 {
            vec![chars[3]]
        } else if height == 2 {
            vec![chars[0], chars[2]]
        } else {
            let mut result = vec![chars[0]];
            for _ in 0..(height - 2) {
                result.push(chars[1]);
            }
            result.push(chars[2]);
            result
        }
    }
}
