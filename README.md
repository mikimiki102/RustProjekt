# Kompresja i dekompresja plików metodą RLE w języku Rust

## Opis projektu

Projekt polega na stworzeniu aplikacji terminalowej w języku Rust do kompresji i dekompresji plików z użyciem algorytmu **RLE (Run-Length Encoding)**.

Program działa z poziomu terminala i umożliwia:
- kompresję pliku wejściowego,
- zapis skompresowanego wyniku do pliku,
- dekompresję wcześniej skompresowanego pliku,
- zapis odtworzonego pliku,
- wyświetlenie podstawowych statystyk dotyczących rozmiaru danych.

## Cel projektu

Celem projektu jest:
- implementacja algorytmu RLE,
- wykorzystanie języka Rust do pracy na plikach i danych binarnych,
- stworzenie aplikacji CLI,
- przeprowadzenie testów poprawności działania programu.

## Algorytm RLE

RLE to metoda bezstratnej kompresji danych polegająca na zapisywaniu serii powtarzających się elementów jako:
- liczba powtórzeń,
- wartość powtarzanego elementu.

### Przykład

Dane wejściowe:
AAAABBBCCDAA

Po kompresji:
4A3B2C1D2A

Dzięki temu można zmniejszyć rozmiar danych, jeśli zawierają długie sekwencje powtórzeń.

## Zakres funkcjonalności

### Wersja podstawowa
- odczyt pliku wejściowego,
- kompresja danych metodą RLE,
- zapis skompresowanego pliku,
- dekompresja pliku skompresowanego,
- zapis pliku odtworzonego,
- obsługa argumentów z terminala,
- podstawowa obsługa błędów.

### Możliwe rozszerzenia
- własny format pliku `.rle`,
- statystyki kompresji,
- dokładniejsze komunikaty błędów,
- testy różnych typów danych,
- analiza skuteczności kompresji dla różnych plików.

## Technologie

- **Rust**
- **Cargo**
- standardowa biblioteka języka Rust
- opcjonalnie biblioteki do obsługi CLI, np. `clap`

## Sposób uruchamiania

### Kompresja
```bash
cargo run -- compress input.txt output.rle
