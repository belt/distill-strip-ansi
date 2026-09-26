package distillstripansi

import "testing"

func TestStrip(t *testing.T) {
	got, err := Strip([]byte("a\x1b[31mb\x1b[0m"))
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != "ab" {
		t.Fatalf("Strip() = %q, want %q", got, "ab")
	}
}

func TestContainsANSI(t *testing.T) {
	for _, test := range []struct {
		input string
		want  bool
	}{
		{input: "plain", want: false},
		{input: "\x1b[31mred", want: true},
	} {
		got, err := ContainsANSI([]byte(test.input))
		if err != nil {
			t.Fatal(err)
		}
		if got != test.want {
			t.Errorf("ContainsANSI(%q) = %t, want %t", test.input, got, test.want)
		}
	}
}

func TestEmptyInput(t *testing.T) {
	got, err := Strip(nil)
	if err != nil {
		t.Fatal(err)
	}
	if len(got) != 0 {
		t.Fatalf("Strip(nil) = %q, want empty", got)
	}
}
