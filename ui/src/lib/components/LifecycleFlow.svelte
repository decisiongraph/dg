<script lang="ts">
	import type { SchemaEnumValue } from '$lib/types';
	import { isDark } from '$lib/stores/theme';

	interface Props {
		nodes: SchemaEnumValue[];
		currentStatus?: string;
		docType?: string;
	}

	let { nodes, currentStatus, docType }: Props = $props();

	let containerEl: HTMLDivElement | undefined = $state();

	/** Map status names to color categories — must match StatusBadge.colorMap exactly */
	const categoryMap: Record<string, string> = {
		active: 'emerald',
		live: 'emerald',
		'in-progress': 'amber',
		accepted: 'emerald',
		resolved: 'emerald',
		implemented: 'emerald',
		approved: 'emerald',
		pursuing: 'amber',
		proposed: 'amber',
		draft: 'amber',
		beta: 'amber',
		validating: 'amber',
		investigating: 'amber',
		review: 'amber',
		open: 'amber',
		exploring: 'amber',
		mitigated: 'amber',
		identified: 'blue',
		planned: 'blue',
		completed: 'slate',
		delivered: 'slate',
		declined: 'red',
		rejected: 'red',
		deprecated: 'red',
		sunset: 'red',
		superseded: 'slate',
		retired: 'red',
		parked: 'slate',
		postmortem: 'slate'
	};

	/** Per-type overrides where same status name needs different color */
	const typeCategoryOverrides: Record<string, Record<string, string>> = {
		inc: { active: 'red', open: 'red' },
		opp: { completed: 'emerald' },
		pol: { proposed: 'blue' },
		spec: { proposed: 'blue', approved: 'amber' }
	};

	function colorCategory(name: string): string {
		return typeCategoryOverrides[docType ?? '']?.[name.toLowerCase()] ??
			categoryMap[name.toLowerCase()] ?? 'slate';
	}

	function capitalize(s: string): string {
		return s.charAt(0).toUpperCase() + s.slice(1);
	}

	function buildMermaidDef(dark: boolean): string {
		const lines: string[] = ['flowchart LR'];
		const cur = currentStatus?.toLowerCase();

		// Define nodes with stadium/pill shape ([...])
		for (const node of nodes) {
			lines.push(`    ${node.name}([${capitalize(node.name)}])`);
		}

		// Add transitions
		for (const node of nodes) {
			if (node.transitions?.length) {
				for (const target of node.transitions) {
					lines.push(`    ${node.name} --> ${target}`);
				}
			}
		}

		// classDef — same light-mode pill colors in both themes
		lines.push('    classDef emerald fill:#d1fae5,stroke:#34d399,color:#065f46');
		lines.push('    classDef amber fill:#fef3c7,stroke:#fbbf24,color:#92400e');
		lines.push('    classDef red fill:#fee2e2,stroke:#f87171,color:#991b1b');
		lines.push('    classDef blue fill:#dbeafe,stroke:#60a5fa,color:#1e40af');
		lines.push('    classDef slate fill:#f1f5f9,stroke:#94a3b8,color:#334155');
		lines.push('    classDef current stroke-width:3px');

		// Group nodes by color and assign classes
		const groups: Record<string, string[]> = {};
		for (const node of nodes) {
			const cat = colorCategory(node.name);
			if (!groups[cat]) groups[cat] = [];
			groups[cat].push(node.name);
		}
		for (const [cat, names] of Object.entries(groups)) {
			lines.push(`    class ${names.join(',')} ${cat}`);
		}

		// Highlight current status
		if (cur) {
			lines.push(`    class ${cur} current`);
		}

		return lines.join('\n');
	}

	async function renderDiagram(el: HTMLDivElement, dark: boolean) {
		const def = buildMermaidDef(dark);
		try {
			const mermaid = (await import('mermaid')).default;
			mermaid.initialize({
				startOnLoad: false,
				theme: dark ? 'dark' : 'default',
				themeVariables: dark ? { darkMode: true } : {},
				flowchart: { useMaxWidth: true, curve: 'basis' }
			});
			const id = `lifecycle-${Math.random().toString(36).slice(2, 9)}`;
			const { svg } = await mermaid.render(id, def);
			el.innerHTML = svg;

			// Constrain SVG for compact look
			const svgEl = el.querySelector('svg');
			if (svgEl) {
				svgEl.style.maxHeight = '120px';
				svgEl.style.width = '100%';
				svgEl.style.height = 'auto';
			}
		} catch (err) {
			console.error('LifecycleFlow mermaid render failed:', err);
			el.innerHTML = '';
		}
	}

	$effect(() => {
		const dark = $isDark;
		const el = containerEl;
		if (!el || nodes.length === 0) return;
		renderDiagram(el, dark);
	});
</script>

<div bind:this={containerEl} class="overflow-x-auto lifecycle-flow"></div>

<style>
	.lifecycle-flow :global(svg) {
		max-height: 120px;
	}
</style>
