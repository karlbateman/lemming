package example

import (
	"fmt"
	"net/http"
)

// New is a factory which returns a service object.
func New(client *http.Client) *Service {
    return &Service{client}
}

// Service wraps a HTTP client dependency.
type Service struct {
    client *http.Client
}

// Get returns the string "example".
func (s *Service) Get() string {
    return "example"
}

func thing() {
    s := New(nil)
    fmt.Println(s.Get())
}
