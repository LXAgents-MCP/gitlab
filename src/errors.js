/** A request the owner would not want served, or one that cannot succeed. Carries a message worth reading. */
export class Refused extends Error {
  constructor(message) {
    super(message);
    this.name = "Refused";
  }
}
