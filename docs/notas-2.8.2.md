# Vortexine's OS 2.8.2

## Lo arreglado

- **Menús a la frecuencia de tu monitor.** Los menús y paneles se movían a unos 60 cuadros por segundo aunque tu monitor
  fuera más rápido. Ahora van a lo que dé cada pantalla: en una prueba con un monitor de 200 Hz y otro de 60 Hz, el
  lanzador pasó de 60 a 200 cuadros en el primero y siguió en 60 en el segundo. Las apps de Vortexine hechas con Qt
  también suben. No tienes que configurar nada.

## Lo que debes saber

- **Usa un poco más de memoria:** unos 47 MB más en el escritorio, porque cada ventana vuelve a dibujarse por su cuenta.
- **Si tu monitor es de 60 Hz, no vas a notar más fluidez:** ya iba a su máximo.

## Para actualizar

```
vortex actualizar
```
