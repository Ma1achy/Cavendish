import math
G=6.67430e-11; hbar=1.054571817e-34; m=1.4432e-25; lam=698e-9
n=1000; k=2*math.pi/lam; keff=n*k; vk=n*hbar*k/m
def carlton13(M,D,h,dr,T):  # Carlton & McCabe eq.13, Gamma=V'' at lower launch point
    return -keff*T**2*G*M*dr*(D*D-2*h*h)/(D*D+h*h)**2.5
print("keff",keff,"vkick",vk)
print("AION-10 10kg D=1km h=1m eq13:",carlton13(10,1000,1,5,0.73))
print("AION-10 10kg D=100m h=1m eq13:",carlton13(10,100,1,5,0.73))
print("AION-10 10kg D=10m h=1m eq13:",carlton13(10,10,1,5,0.73))
# AICE TP: dr=70,T=2.5
print("AICE 10kg D=1km h=1 eq13:",carlton13(10,1000,1,70,2.5))
print("ratio AICE/AION far field",carlton13(10,1000,1,70,2.5)/carlton13(10,1000,1,5,0.73))
# launch rule u0=gT - vk/2
g=9.81
for name,T in (("AION-10",0.73),("AICE",2.5)):
    u0=g*T-vk/2; print(name,"u0",u0)
# Earth-gradient recoil phase (single IFO, uniform Tzz): -(1/2) n k dz Tzz T^2, dz=vk*T  (Asenbaum 2017)
Tzz=3.086e-6
for name,T in (("AION-10",0.73),("AICE",2.5)):
    print(name,"recoil phase per IFO (rad)",0.5*keff*vk*T*T*Tzz*T/T*1.0)
# upper-arm apex of AICE (dr=70): launch u0+vk (upper arm) -> engine draft said 38.9
u0=g*2.5-vk/2
print("apex above launch if arm speed u0+vk/2:",(u0+vk/2)**2/(2*g)," if u0+vk:",(u0+vk)**2/(2*g))
