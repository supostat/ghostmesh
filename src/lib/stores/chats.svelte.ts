import { listChats, getChat, type ChatInfo, type ChatDetail } from "../api";

let chatList = $state<ChatInfo[]>([]);
let selectedChatId = $state<string | null>(null);
let selectedChatDetail = $state<ChatDetail | null>(null);
let loading = $state(false);
let error = $state<string | null>(null);

export function getChatList() {
  return chatList;
}

export function getSelectedChatId() {
  return selectedChatId;
}

export function getSelectedChatDetail() {
  return selectedChatDetail;
}

export function isChatLoading() {
  return loading;
}

export function getChatError() {
  return error;
}

export async function loadChats(): Promise<void> {
  loading = true;
  error = null;
  try {
    chatList = await listChats();
  } catch (err) {
    error = String(err);
  } finally {
    loading = false;
  }
}

export async function selectChat(chatId: string): Promise<void> {
  selectedChatId = chatId;
  try {
    selectedChatDetail = await getChat(chatId);
  } catch (err) {
    error = String(err);
    selectedChatDetail = null;
  }
}

export function clearSelectedChat(): void {
  selectedChatId = null;
  selectedChatDetail = null;
}

export function updateChatInList(updated: ChatInfo): void {
  const index = chatList.findIndex((c) => c.chat_id === updated.chat_id);
  if (index >= 0) {
    chatList[index] = updated;
  } else {
    chatList = [...chatList, updated];
  }
}

export function removeChatFromList(chatId: string): void {
  chatList = chatList.filter((c) => c.chat_id !== chatId);
  if (selectedChatId === chatId) {
    selectedChatId = null;
    selectedChatDetail = null;
  }
}
