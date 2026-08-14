export type ConfirmationRequest = {
  title: string;
  detail: string;
  confirmLabel: string;
  danger?: boolean;
};

type ConfirmationHandler = (request: ConfirmationRequest) => Promise<boolean>;

let handler: ConfirmationHandler | null = null;

export function setConfirmationHandler(next: ConfirmationHandler | null): void {
  handler = next;
}

export function requestConfirmation(request: ConfirmationRequest): Promise<boolean> {
  return handler ? handler(request) : Promise.resolve(false);
}
