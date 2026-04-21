export type ClientMessageType = "message.send" | "media.send" | "ping";

export type ServerMessageType =
  | "message.create"
  | "message.update"
  | "media.create"
  | "typing.start"
  | "typing.stop"
  | "error"
  | "pong";

export type HermesMessageType = ClientMessageType | ServerMessageType;

export interface HermesMessage<P = Record<string, unknown>> {
  type: HermesMessageType;
  id?: string;
  session_id?: string;
  timestamp?: number;
  payload?: P;
}

export interface MessageCreatePayload {
  message_id: string;
  content: string;
  thought?: boolean;
}

export interface MessageUpdatePayload extends MessageCreatePayload {}

export interface MessageSendPayload {
  content: string;
  media?: string | object | unknown[];
}

export interface ErrorPayload {
  code: string;
  message: string;
  request_id?: string;
}

const SERVER_TYPES = new Set<ServerMessageType>([
  "message.create",
  "message.update",
  "media.create",
  "typing.start",
  "typing.stop",
  "error",
  "pong",
]);

export function isServerMessage(m: HermesMessage): boolean {
  return SERVER_TYPES.has(m.type as ServerMessageType);
}
