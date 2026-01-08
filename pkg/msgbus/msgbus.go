package msgbus

import (
	"sync"
)

type EventType string

const (
	EventStatusChanged EventType = "status_changed"
	EventError         EventType = "error"
	EventHealthCheck   EventType = "health_check"
)

type Event struct {
	Type    EventType
	Payload interface{}
}

type Listener func(Event)

type MsgBus struct {
	listeners map[EventType][]Listener
	mu        sync.RWMutex
}

var (
	instance *MsgBus
	once     sync.Once
)

func GetBus() *MsgBus {
	once.Do(func() {
		instance = &MsgBus{
			listeners: make(map[EventType][]Listener),
		}
	})
	return instance
}

func (b *MsgBus) Subscribe(t EventType, l Listener) {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.listeners[t] = append(b.listeners[t], l)
}

func (b *MsgBus) Publish(e Event) {
	b.mu.RLock()
	defer b.mu.RUnlock()
	if listeners, ok := b.listeners[e.Type]; ok {
		for _, l := range listeners {
			go l(e)
		}
	}
}
