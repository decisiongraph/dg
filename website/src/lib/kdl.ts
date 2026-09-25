// Minimal KDL (v1 + v2 syntax) parser: enough to read dg's schema.kdl and
// org.kdl at build time. Supports nodes, positional args, key=value props,
// children blocks, quoted/raw/multi-line strings, numbers, #true/#false/#null
// (and v1 true/false/null), `//` `/* */` comments, `/-` slashdash, `;`
// terminators and `\` line continuations. Type annotations `(t)` are skipped.

export type KdlValue = string | number | boolean | null;

export interface KdlNode {
	name: string;
	args: KdlValue[];
	props: Record<string, KdlValue>;
	children: KdlNode[];
}

export class KdlError extends Error {
	constructor(message: string, src: string, pos: number) {
		const line = src.slice(0, pos).split('\n').length;
		super(`KDL parse error at line ${line}: ${message}`);
	}
}

const IDENT_STOP = new Set([...'(){}[]/\\"#;=', ' ', '\t', '\n', '\r', '\f', '﻿']);

export function parseKdl(src: string): KdlNode[] {
	let i = 0;

	const fail = (msg: string): never => {
		throw new KdlError(msg, src, i);
	};

	// Skip spaces, comments and escaped newlines; stop at newline unless `nl`.
	const skipWs = (nl: boolean) => {
		for (;;) {
			const c = src[i];
			if (c === ' ' || c === '\t' || c === '﻿' || c === '\f') i++;
			else if (nl && (c === '\n' || c === '\r' || c === ';')) i++;
			else if (c === '/' && src[i + 1] === '/') {
				while (i < src.length && src[i] !== '\n') i++;
			} else if (c === '/' && src[i + 1] === '*') {
				let depth = 0;
				do {
					if (src.startsWith('/*', i)) {
						depth++;
						i += 2;
					} else if (src.startsWith('*/', i)) {
						depth--;
						i += 2;
					} else if (i >= src.length) fail('unterminated block comment');
					else i++;
				} while (depth > 0);
			} else if (c === '\\') {
				// line continuation
				i++;
				skipWs(false);
				if (src[i] === '\r') i++;
				if (src[i] === '\n') i++;
			} else return;
		}
	};

	const readQuoted = (): string => {
		// multi-line """ strings (v2): dedent by the closing line's indent
		if (src.startsWith('"""', i)) {
			i += 3;
			const end = src.indexOf('"""', i);
			if (end < 0) fail('unterminated multi-line string');
			const body = src.slice(i, end);
			i = end + 3;
			const lines = body.replace(/^\r?\n/, '').split(/\r?\n/);
			const indent = lines.pop() ?? '';
			return lines.map((l) => (l.startsWith(indent) ? l.slice(indent.length) : l)).join('\n');
		}
		i++; // opening quote
		let out = '';
		while (i < src.length && src[i] !== '"') {
			const c = src[i++];
			if (c !== '\\') {
				out += c;
				continue;
			}
			const e = src[i++];
			const simple: Record<string, string> = {
				n: '\n',
				r: '\r',
				t: '\t',
				'\\': '\\',
				'/': '/',
				'"': '"',
				b: '\b',
				f: '\f',
				s: ' '
			};
			if (e in simple) out += simple[e];
			else if (e === 'u' && src[i] === '{') {
				const end = src.indexOf('}', i);
				out += String.fromCodePoint(parseInt(src.slice(i + 1, end), 16));
				i = end + 1;
			} else if (/\s/.test(e)) {
				while (/\s/.test(src[i] ?? '')) i++; // whitespace escape
			} else fail(`bad escape \\${e}`);
		}
		if (src[i] !== '"') fail('unterminated string');
		i++;
		return out;
	};

	const readRaw = (): string => {
		// v2: #"..."#, v1: r#"..."#
		if (src[i] === 'r') i++;
		let hashes = 0;
		while (src[i] === '#') {
			hashes++;
			i++;
		}
		if (src[i] !== '"') fail('bad raw string');
		const multi = src.startsWith('"""', i);
		const open = multi ? 3 : 1;
		const close = (multi ? '"""' : '"') + '#'.repeat(hashes);
		const end = src.indexOf(close, i + open);
		if (end < 0) fail('unterminated raw string');
		const body = src.slice(i + open, end);
		i = end + close.length;
		return body;
	};

	const readBare = (): string => {
		const start = i;
		while (i < src.length && !IDENT_STOP.has(src[i])) i++;
		if (i === start) fail(`unexpected character ${JSON.stringify(src[i])}`);
		return src.slice(start, i);
	};

	const readString = (): string => {
		if (src[i] === '"') return readQuoted();
		if (src[i] === '#' && (src[i + 1] === '"' || src[i + 1] === '#')) return readRaw();
		if (src[i] === 'r' && (src[i + 1] === '"' || src[i + 1] === '#')) return readRaw();
		return readBare();
	};

	const toValue = (raw: string, quoted: boolean): KdlValue => {
		if (quoted) return raw;
		switch (raw) {
			case '#true':
			case 'true':
				return true;
			case '#false':
			case 'false':
				return false;
			case '#null':
			case 'null':
				return null;
		}
		const num = raw.replace(/_/g, '');
		if (/^[+-]?(0x[0-9a-f]+|0o[0-7]+|0b[01]+)$/i.test(num)) {
			const neg = num.startsWith('-');
			const n = Number(num.replace(/^[+-]/, ''));
			return neg ? -n : n;
		}
		if (/^[+-]?\d+(\.\d+)?(e[+-]?\d+)?$/i.test(num)) return Number(num);
		return raw;
	};

	const skipTypeAnnotation = () => {
		if (src[i] === '(') {
			const end = src.indexOf(')', i);
			if (end < 0) fail('unterminated type annotation');
			i = end + 1;
		}
	};

	const readValue = (): KdlValue => {
		skipTypeAnnotation();
		const quoted = src[i] === '"' || src[i] === '#' || (src[i] === 'r' && /[#"]/.test(src[i + 1]));
		const isKeyword = src[i] === '#' && /[a-z-]/i.test(src[i + 1] ?? '');
		if (isKeyword) {
			i++;
			return toValue('#' + readBare(), false);
		}
		const raw = readString();
		return toValue(raw, quoted);
	};

	const parseNodes = (inBlock: boolean): KdlNode[] => {
		const nodes: KdlNode[] = [];
		for (;;) {
			skipWs(true);
			if (i >= src.length) {
				if (inBlock) fail('unterminated children block');
				return nodes;
			}
			if (src[i] === '}') {
				if (!inBlock) fail('unexpected }');
				i++;
				return nodes;
			}
			let slashdash = false;
			if (src.startsWith('/-', i)) {
				slashdash = true;
				i += 2;
				skipWs(true);
			}
			const node = parseNode();
			if (!slashdash) nodes.push(node);
		}
	};

	const parseNode = (): KdlNode => {
		skipTypeAnnotation();
		const node: KdlNode = { name: readString(), args: [], props: {}, children: [] };
		for (;;) {
			skipWs(false);
			const c = src[i];
			if (c === undefined || c === '\n' || c === '\r' || c === ';' || c === '}') {
				if (c === ';') i++;
				return node;
			}
			let slashdash = false;
			if (src.startsWith('/-', i)) {
				slashdash = true;
				i += 2;
				skipWs(false);
			}
			if (src[i] === '{') {
				i++;
				const children = parseNodes(true);
				if (!slashdash) node.children.push(...children);
				continue;
			}
			// property or argument
			const start = i;
			const bareKey = src[i] !== '"' && src[i] !== '#' && src[i] !== '(';
			let key: string | null = null;
			if (bareKey || src[i] === '"') {
				const k = readString();
				if (src[i] === '=') {
					key = k;
					i++;
				} else i = start;
			}
			const value = readValue();
			if (slashdash) continue;
			if (key !== null) node.props[key] = value;
			else node.args.push(value);
		}
	};

	return parseNodes(false);
}
