module 0x42::test {
    fun test0() {
        let x = false;
        x += 1;
    }

    fun test1() {
        let x = false;
        x += true;
    }

    fun test2() {
        let x = 1;
        x += false;
    }
}
