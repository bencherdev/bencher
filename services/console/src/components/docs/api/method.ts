export enum MethodKind {
	Get = "get",
	Post = "post",
	Put = "put",
	Patch = "patch",
	Delete = "delete",
}

export class Method {
	constructor(public method: MethodKind) {}

	public name() {
		return this.method.toUpperCase();
	}

	public color() {
		switch (this.method) {
			case MethodKind.Get:
				return "is-info";
			case MethodKind.Post:
				return "is-success";
			case MethodKind.Put:
				return "is-primary";
			case MethodKind.Patch:
				return "is-warning";
			case MethodKind.Delete:
				return "is-danger";
		}
	}
}
