
\pagenumbering{gobble}
Claro que sí, ¡me encanta resolver problemas matemáticos!

Para demostrar que la norma $P$ converge a la norma infinita en un espacio vectorial sobre $\mathbb{R}^n$ cuando $P$ tiende a infinito, es necesario demostrar que la tendencia se cumple para cualquier vector $\textbf{x} \in \mathbb{R}^n$. En otras palabras, debemos mostrar que:

$$\lim_{P\to\infty}\|\textbf{x}\|_P = \|\textbf{x}\|_\infty$$

Donde $\|\textbf{x}\|_P$ es la norma $P$ del vector $\textbf{x}$ y $\|\textbf{x}\|_\infty$ es la norma infinita del vector.

La norma $P$ se define como:

$$\|\textbf{x}\|_P = \left(\sum_{i=1}^n |x_i|^P\right)^{1/P}$$

Mientras que la norma infinita se define como:

$$\|\textbf{x}\|_\infty = \max_{1\leq i \leq n} |x_i|$$

Para empezar, notemos que $\|\textbf{x}\|_\infty \leq \|\textbf{x}\|_P$ para cualquier valor de $P$. Esto es simplemente una aplicación de la desigualdad de Hölder, que establece que para cualquier $p,q > 0$ tales que $\frac{1}{p} + \frac{1}{q} = 1$, se cumple que:

$$\sum_{i=1}^n |a_ib_i| \leq \left(\sum_{i=1}^n |a_i|^p\right)^{1/p}\left(\sum_{i=1}^n |b_i|^q\right)^{1/q}$$

Tomando $a_i = |x_i|^{1-p}$ y $b_i = |x_i|^P$, podemos escribir:

$$\|\textbf{x}\|_P = \left(\sum_{i=1}^n |x_i|^P\right)^{1/P} = \left(\sum_{i=1}^n |x_i|^{P(1-p)}\right)^{1/P}\left(\sum_{i=1}^n|x_i|^p\right)^{1/p}$$

Usando la desigualdad de Hölder con $p = P$ y $q = \frac{P}{P-1}$, obtenemos:

$$\|\textbf{x}\|_P \geq \left(\sum_{i=1}^n |x_i|^{P(1-p)}\right)^{1/P}|x_k|$$

Donde $k$ es el índice para el cual $|x_k| = \|\textbf{x}\|_\infty$. Observamos que $\sum_{i=1}^n |x_i|^{P(1-p)} \to \infty$ cuando $P \to \infty$, ya que el término $|x_k|^{P(1-p)}$ domina sobre los demás términos de la suma. Por lo tanto, tenemos que:

$$\lim_{P\to\infty}\|\textbf{x}\|_P \geq \lim_{P\to\infty}\left(\sum_{i=1}^n |x_i|^{P(1-p)}\right)^{1/P}|x_k| = |x_k| = \|\textbf{x}\|_\infty$$

Por otro lado, podemos escribir $\|\textbf{x}\|_P$ en términos de $\|\textbf{x}\|_\infty$ como:

$$\|\textbf{x}\|_P \leq n^{1/P}\max_{1\leq i \leq n} |x_i| = n^{1/P}\|\textbf{x}\|_\infty$$

Notamos que $\lim_{P\to\infty} n^{1/P} = 1$, por lo cual tenemos que:

$$\lim_{P\to\infty}\|\textbf{x}\|_P \leq \lim_{P\to\infty} n^{1/P}\|\textbf{x}\|_\infty = \|\textbf{x}\|_\infty$$

En conclusión, hemos demostrado que:

$$\lim_{P\to\infty}\|\textbf{x}\|_P = \|\textbf{x}\|_\infty$$

Para cualquier vector $\textbf{x} \in \mathbb{R}^n$. Esto prueba que la norma $P$ converge a la norma infinita en un espacio vectorial sobre $\mathbb{R}^n$ cuando $P$ tiende a infinito. 