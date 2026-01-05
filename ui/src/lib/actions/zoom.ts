import { select } from 'd3-selection';
import { zoom, zoomIdentity, type ZoomBehavior } from 'd3-zoom';

export function zoomable(node: SVGSVGElement) {
	const svg = select(node);
	const g = svg.select<SVGGElement>('g.zoom-group');

	const zoomBehavior: ZoomBehavior<SVGSVGElement, unknown> = zoom<SVGSVGElement, unknown>()
		.scaleExtent([0.1, 4])
		.on('zoom', (event) => {
			g.attr('transform', event.transform.toString());
		});

	svg.call(zoomBehavior);

	// Fit content on initial render
	requestAnimationFrame(() => {
		const bbox = g.node()?.getBBox();
		if (!bbox || bbox.width === 0) return;

		const pad = 40;
		const fullWidth = node.clientWidth || 800;
		const fullHeight = node.clientHeight || 600;
		const scale = Math.min(
			fullWidth / (bbox.width + pad * 2),
			fullHeight / (bbox.height + pad * 2),
			1
		);
		const tx = (fullWidth - bbox.width * scale) / 2 - bbox.x * scale;
		const ty = (fullHeight - bbox.height * scale) / 2 - bbox.y * scale;

		svg.call(zoomBehavior.transform, zoomIdentity.translate(tx, ty).scale(scale));
	});

	return {
		destroy() {
			svg.on('.zoom', null);
		}
	};
}
