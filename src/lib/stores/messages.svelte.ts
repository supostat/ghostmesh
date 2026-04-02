import { getMessages, type MessageInfo, type DeliveryStatus } from "../api";

let messages = $state<MessageInfo[]>([]);
let loading = $state(false);
let error = $state<string | null>(null);
let currentChatId = $state<string | null>(null);

export function getMessageList() {
  return messages;
}

export function isMessagesLoading() {
  return loading;
}

export function getMessagesError() {
  return error;
}

export function getMessagesChatId() {
  return currentChatId;
}

export async function loadMessages(
  chatId: string,
  password: string,
): Promise<void> {
  currentChatId = chatId;
  loading = true;
  error = null;
  try {
    messages = await getMessages(chatId, password);
  } catch (err) {
    error = String(err);
  } finally {
    loading = false;
  }
}

export function appendMessage(message: MessageInfo): void {
  if (message.chat_id !== currentChatId) return;
  const exists = messages.some(
    (m) => m.message_id === message.message_id,
  );
  if (!exists) {
    messages = [...messages, message];
  }
}

export function updateDeliveryStatus(
  messageId: string,
  status: DeliveryStatus,
): void {
  const index = messages.findIndex((m) => m.message_id === messageId);
  if (index >= 0) {
    messages[index] = { ...messages[index], delivery_status: status };
  }
}

export function clearMessages(): void {
  messages = [];
  currentChatId = null;
  error = null;
}
