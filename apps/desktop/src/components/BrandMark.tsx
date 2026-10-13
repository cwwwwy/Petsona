import brand from "../assets/brand.png";
export function BrandMark({ large = false }: {
    large?: boolean;
}) {
    return <img className={`brand-mark${large ? " large" : ""}`} src={brand} alt="" aria-hidden="true"/>;
}
