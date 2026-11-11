/**
 * The type of an answer from GitLab.
 *
 * A body is JSON that came over the network, from an API that adds fields over time and from a
 * proxy or an outage that can send anything at all. A hand-written interface per endpoint would be
 * a claim nothing checks, so a body stays loosely typed where it enters, and every tool checks the
 * one field it relies on (`expectBody` in `shapes.ts`) before it reads the rest.
 */
export type ApiBody = any;
