from .views import accept, bindings, collect

KINDS = {"messages", "chats", "message-history", "message-links", "message-attachments"}

__all__ = ["KINDS", "collect", "accept", "bindings"]
