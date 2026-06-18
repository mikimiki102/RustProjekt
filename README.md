# Kompresja i dekompresja plików metodą RLE w języku Rust

## Opis projektu

Projekt polega na stworzeniu aplikacji terminalowej w języku Rust do kompresji i dekompresji plików z użyciem algorytmu **RLE (Run-Length Encoding)**.

Program działa z poziomu terminala i umożliwia:

* kompresję pliku wejściowego,
* zapis skompresowanego wyniku do pliku,
* dekompresję wcześniej skompresowanego pliku,
* zapis odtworzonego pliku,
* analizę pliku wejściowego,
* automatyczny wybór metody kompresji,
* wyświetlenie podstawowych statystyk dotyczących rozmiaru danych i czasu działania.

## Cel projektu

Celem projektu jest:

* implementacja algorytmu RLE,
* wykorzystanie języka Rust do pracy na plikach i danych binarnych,
* stworzenie aplikacji CLI,
* obsługa kompresji na poziomie bajtów i bitów,
* przygotowanie prostego analizatora struktury pliku,
* przeprowadzenie testów poprawności działania programu.

## Algorytm RLE

RLE to metoda bezstratnej kompresji danych polegająca na zapisywaniu serii powtarzających się elementów jako:

* liczba powtórzeń,
* wartość powtarzanego elementu.

### Przykład

Dane wejściowe:

```text
AAAABBBCCDAA
```

Po kompresji:

```text
4A3B2C1D2A
```

Dzięki temu można zmniejszyć rozmiar danych, jeśli zawierają długie sekwencje powtórzeń.

W programie dane są przetwarzane binarnie, więc wynik nie jest zapisywany jako tekst `4A3B2C1D2A`, tylko jako pary bajtów, np.:

```text
[4, A, 3, B, 2, C, 1, D, 2, A]
```

## Rodzaje kompresji

Program obsługuje dwa warianty kompresji RLE.

### Kompresja bajtowa

Kompresja bajtowa analizuje kolejne bajty pliku i zapisuje serie powtarzających się bajtów jako pary:

```text
[liczba_powtórzeń, bajt]
```

Przykład:

```text
AAAAABBB
```

zostaje zapisane jako:

```text
[5, A, 3, B]
```

Jest to podstawowa wersja algorytmu RLE.

### Kompresja bitowa

Kompresja bitowa działa podobnie, ale zamiast całych bajtów analizuje pojedyncze bity `0` i `1`.

W tej wersji jeden bajt skompresowanych danych przechowuje:

```text
najstarszy bit      -> wartość powtarzanego bitu
pozostałe 7 bitów   -> liczba powtórzeń
```

Dzięki temu możliwe jest kompresowanie danych, w których powtarzają się nie całe bajty, ale dłuższe serie bitów.

## Analizator pliku

W projekcie dodano prosty analizator struktury pliku.

Jego zadaniem jest sprawdzenie próbki pliku wejściowego i zaproponowanie bardziej korzystnej metody kompresji:

* kompresji bajtowej,
* albo kompresji bitowej.

Analizator liczy średnią długość serii powtarzających się bajtów oraz średnią długość serii powtarzających się bitów. Następnie wybiera metodę, dla której potencjalna kompresja powinna być korzystniejsza.

## Zakres funkcjonalności

Aktualna wersja programu obsługuje:

* odczyt pliku wejściowego,
* kompresję danych metodą RLE,
* kompresję bajtową,
* kompresję bitową,
* automatyczny wybór metody kompresji,
* zapis skompresowanego pliku,
* zapis nagłówka z informacją o użytej metodzie kompresji,
* dekompresję pliku skompresowanego,
* odczyt nagłówka podczas dekompresji,
* zapis pliku odtworzonego,
* obsługę argumentów z terminala,
* podstawową obsługę błędów,
* wyświetlanie statystyk rozmiaru i czasu działania,
* testy jednostkowe.

## Struktura projektu

Przykładowa struktura projektu:

```text
src/
├── main.rs
└── core/
    ├── mod.rs
    ├── memcompress.rs
    └── file/
        ├── mod.rs
        ├── filecompress.rs
        ├── fileanalyze.rs
        └── fileformat.rs
```

Najważniejsze pliki:

* `main.rs` - obsługa argumentów programu i uruchamianie odpowiednich trybów,
* `memcompress.rs` - funkcje kompresji i dekompresji danych znajdujących się w pamięci,
* `filecompress.rs` - kompresja i dekompresja plików z użyciem przetwarzania strumieniowego,
* `fileanalyze.rs` - analiza pliku i wybór sugerowanej metody kompresji,
* `fileformat.rs` - warstwa łącząca analizator, kompresor i obsługę formatu pliku.

## Technologie

* **Rust**
* **Cargo**
* standardowa biblioteka języka Rust
* testy jednostkowe
* biblioteki pomocnicze używane w testach, m.in. `rand` i `tempfile`

## Sposób uruchamiania

### Analiza pliku

```bash
cargo run -- analyze input_file
```

Przykład:

```bash
cargo run -- analyze assets/example/ex_bin_a
```

### Kompresja

```bash
cargo run -- compress input_file output.rle
```

Przykład:

```bash
cargo run -- compress assets/example/ex_bin_a compressed_a.rle
```

### Dekompresja

```bash
cargo run -- decompress input.rle output_file
```

Przykład:

```bash
cargo run -- decompress compressed_a.rle restored_a.bin
```

### Porównanie plików po dekompresji

Na Windowsie dla plików binarnych można użyć:

```powershell
cmd /c fc /b assets\example\ex_bin_a restored_a.bin
```

Jeżeli pliki są identyczne, program wypisze:

```text
FC: no differences encountered
```

Można też porównać hashe SHA256:

```powershell
certutil -hashfile assets\example\ex_bin_a SHA256
certutil -hashfile restored_a.bin SHA256
```

## Testy

Testy jednostkowe można uruchomić komendą:

```bash
cargo test
```

W aktualnej wersji projektu wszystkie testy przechodzą poprawnie:

```text
test result: ok. 30 passed; 0 failed
```

Testy sprawdzają między innymi:

* kompresję bajtową,
* dekompresję bajtową,
* kompresję bitową,
* dekompresję bitową,
* działanie na pustych danych,
* działanie na danych bez powtórzeń,
* działanie na długich seriach danych,
* poprawność analizatora pliku,
* obsługę błędnych ustawień,
* kompresję i dekompresję z użyciem plików tymczasowych.

## Wnioski

Program poprawnie realizuje kompresję i dekompresję metodą RLE. Działa zarówno dla kompresji bajtowej, jak i bitowej. Dodatkowo analizator pliku pozwala automatycznie dobrać metodę kompresji na podstawie próbki danych.

Wyniki testów pokazują jednak, że algorytm RLE nie zawsze zmniejsza rozmiar pliku. Dla testowanych plików binarnych rozmiar po kompresji był większy od rozmiaru wejściowego. Jest to typowe ograniczenie algorytmu RLE, ponieważ metoda ta działa najlepiej dla danych zawierających długie serie powtarzających się wartości.

Jeśli dane nie zawierają wielu powtórzeń, zapis liczników i wartości może zwiększyć rozmiar pliku. Mimo tego dekompresja pozostaje poprawna, a algorytm jest bezstratny.


## Autorzy
Szymon Bełz,
Mikołaj Wałek
